//! 添付の認可は予定と同じ。管理用・公開共有用の迂回ルートは作らない。
use actix_web::{HttpResponse, delete, get, post, web};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{
    GuildMember,
    events::{EventPath, ensure_can_edit},
};
use crate::{
    attachments::{AttachmentStorage, SignedUrl},
    error::{ApiError, ErrorBody},
    models::{
        attachments::{self, Attachment, UploadInput},
        events,
    },
    state::AppState,
};

#[derive(Serialize, ToSchema)]
pub struct AttachmentLimits {
    enabled: bool,
    file_max_bytes: i64,
    event_max_files: i64,
    guild_max_bytes: i64,
    used_bytes: i64,
}

#[derive(Serialize, ToSchema)]
pub struct UploadReservation {
    id: String,
    upload: SignedUrl,
}

#[derive(Deserialize, IntoParams)]
pub struct AttachmentPath {
    #[allow(dead_code)] // GuildMemberが検証。OpenAPIのパス定義にも必要。
    pub guild_id: String,
    pub event_id: i32,
    pub attachment_id: String,
}

#[derive(Deserialize, IntoParams)]
pub struct ReadQuery {
    #[serde(default)]
    preview: bool,
}

fn json(value: impl Serialize) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "no-store"))
        .json(value)
}

#[utoipa::path(tag="attachments", responses((status=200,body=AttachmentLimits),(status=401,body=ErrorBody),(status=403,body=ErrorBody)))]
#[get("/{guild_id}/attachments")]
pub async fn limits(
    member: GuildMember,
    state: web::Data<AppState>,
    storage: web::Data<AttachmentStorage>,
) -> Result<HttpResponse, ApiError> {
    Ok(json(AttachmentLimits {
        enabled: storage.0.is_some(),
        file_max_bytes: attachments::FILE_MAX_BYTES,
        event_max_files: attachments::EVENT_MAX_FILES,
        guild_max_bytes: attachments::GUILD_MAX_BYTES,
        used_bytes: attachments::used_bytes(&state.pool, member.guild_id()).await?,
    }))
}

#[utoipa::path(tag="attachments",params(EventPath),responses((status=200,body=Vec<Attachment>),(status=401,body=ErrorBody),(status=403,body=ErrorBody),(status=404,body=ErrorBody)))]
#[get("/{guild_id}/{event_id}/attachments")]
pub async fn list(
    member: GuildMember,
    path: web::Path<EventPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    events::find_by_id(&state.pool, member.guild_id(), path.event_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    Ok(json(
        attachments::list(&state.pool, member.guild_id(), path.event_id).await?,
    ))
}

#[utoipa::path(tag="attachments",params(EventPath),request_body=UploadInput,responses((status=200,body=UploadReservation),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=403,body=ErrorBody),(status=404,body=ErrorBody),(status=503,body=ErrorBody)))]
#[post("/{guild_id}/{event_id}/attachments")]
pub async fn reserve(
    member: GuildMember,
    path: web::Path<EventPath>,
    input: web::Json<UploadInput>,
    state: web::Data<AppState>,
    storage: web::Data<AttachmentStorage>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let store = storage.get()?;
    let row = attachments::reserve(&state.pool, member.guild_id(), path.event_id, &input).await?;
    let upload = store.upload_url(&row).await?;
    Ok(json(UploadReservation { id: row.id, upload }))
}

#[utoipa::path(tag="attachments",params(AttachmentPath),responses((status=200,body=Attachment),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=403,body=ErrorBody),(status=404,body=ErrorBody),(status=503,body=ErrorBody)))]
#[post("/{guild_id}/{event_id}/attachments/{attachment_id}/complete")]
pub async fn complete(
    member: GuildMember,
    path: web::Path<AttachmentPath>,
    state: web::Data<AppState>,
    storage: web::Data<AttachmentStorage>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let store = storage.get()?;
    let mut tx = state.pool.begin().await?;
    attachments::lock_event(&mut tx, member.guild_id(), path.event_id).await?;
    let mut row = attachments::find(
        &mut tx,
        member.guild_id(),
        path.event_id,
        &path.attachment_id,
    )
    .await?;
    if !row.ready {
        if row.expires_at <= Utc::now() {
            return Err(ApiError::BadRequest(
                "添付の予約期限が切れました。ファイルを選び直してください".into(),
            ));
        }
        store.verify_and_copy(&row).await?;
        sqlx::query("UPDATE event_attachments SET ready=true WHERE id=$1")
            .bind(&row.id)
            .execute(&mut *tx)
            .await?;
        // URL失効前の再送で一時キーが復活しないよう、失効後に回収する。
        sqlx::query("INSERT INTO attachment_deletions(object_key,next_attempt_at) VALUES ($1,GREATEST(now(),$2 + INTERVAL '15 minutes')) ON CONFLICT DO NOTHING")
            .bind(&row.temporary_key).bind(row.created_at).execute(&mut *tx).await?;
        row.ready = true;
    }
    tx.commit().await?;
    Ok(json(row))
}

#[utoipa::path(tag="attachments",params(AttachmentPath,ReadQuery),responses((status=200,body=SignedUrl),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=403,body=ErrorBody),(status=404,body=ErrorBody),(status=503,body=ErrorBody)))]
#[post("/{guild_id}/{event_id}/attachments/{attachment_id}/url")]
pub async fn read_url(
    member: GuildMember,
    path: web::Path<AttachmentPath>,
    query: web::Query<ReadQuery>,
    state: web::Data<AppState>,
    storage: web::Data<AttachmentStorage>,
) -> Result<HttpResponse, ApiError> {
    let mut tx = state.pool.begin().await?;
    attachments::lock_event(&mut tx, member.guild_id(), path.event_id).await?;
    let row = attachments::find(
        &mut tx,
        member.guild_id(),
        path.event_id,
        &path.attachment_id,
    )
    .await?;
    if !row.ready {
        return Err(ApiError::NotFound("attachment not found".into()));
    }
    Ok(json(storage.get()?.read_url(&row, query.preview).await?))
}

#[utoipa::path(tag="attachments",params(AttachmentPath),responses((status=204),(status=401,body=ErrorBody),(status=403,body=ErrorBody),(status=404,body=ErrorBody)))]
#[delete("/{guild_id}/{event_id}/attachments/{attachment_id}")]
pub async fn remove(
    member: GuildMember,
    path: web::Path<AttachmentPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let mut tx = state.pool.begin().await?;
    attachments::lock_event(&mut tx, member.guild_id(), path.event_id).await?;
    sqlx::query("DELETE FROM event_attachments WHERE guild_id=$1 AND event_id=$2 AND id=$3")
        .bind(member.guild_id())
        .bind(path.event_id)
        .bind(&path.attachment_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}
