//! 添付の認可は予定と同じ。管理用・公開共有用の迂回ルートは作らない。
use actix_web::{HttpResponse, delete, get, post, put, web};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::{
    GuildMember,
    events::{EventPath, describe_scheduled_event_error, ensure_can_edit},
};
use crate::{
    attachments::{AttachmentStorage, SignedUrl},
    discord::DiscordError,
    error::{ApiError, ErrorBody},
    models::{
        attachments::{self, Attachment, UploadInput},
        event_links, events,
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
    storage: web::Data<AttachmentStorage>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let guild_id = member.guild_id();
    let _event_lock = state.event_update_locks.lock(path.event_id).await;
    let mut tx = state.pool.begin().await?;
    attachments::lock_event(&mut tx, guild_id, path.event_id).await?;
    let was_cover: Option<bool> = sqlx::query_scalar(
        "DELETE FROM event_attachments WHERE guild_id=$1 AND event_id=$2 AND id=$3 RETURNING discord_cover",
    )
    .bind(guild_id)
    .bind(path.event_id)
    .bind(&path.attachment_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    // カバーだった添付を消したら Discord 側の画像も外す (#403)。連携の解除後の削除と同じく
    // ベストエフォートで、失敗しても添付の削除は取り消さない
    if was_cover == Some(true)
        && let Err(err) = sync_cover(&state, &storage, guild_id, path.event_id).await
    {
        tracing::warn!(
            guild_id,
            event_id = path.event_id,
            kind = err.kind(),
            "failed to clear a cover image in discord"
        );
    }
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize, ToSchema)]
pub struct CoverInput {
    /// カバーにする添付の ID。`null` でカバーを外す
    attachment_id: Option<String>,
}

/// Discord への反映結果 (#403)。カバーの保存は反映の成否に関わらず確定している
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoverSyncStatus {
    /// Discord のイベントに反映した
    Synced,
    /// 予定が Discord のイベントと連携していない (次に連携したときに使う)
    NotLinked,
    /// 反映に失敗した。同じ内容で送り直せば再試行になる
    Failed,
}

#[derive(Serialize, ToSchema)]
pub struct CoverSync {
    status: CoverSyncStatus,
    /// `failed` のときの理由 (通常のエラーレスポンスと同じ形)
    error: Option<ErrorBody>,
}

#[derive(Serialize, ToSchema)]
pub struct CoverResult {
    /// 更新後の添付の一覧
    attachments: Vec<Attachment>,
    discord: CoverSync,
}

/// 予定の添付のうち 1 枚を Discord スケジュールイベントのカバーにする (#403)。
///
/// カバーの選択を先に保存し、その後で連携中の Discord イベントに反映する。Discord への反映が
/// 失敗してもカバーの保存は取り消さず、200 の本文 (`discord.status = failed`) で知らせる
/// (予定や添付の保存と同じく、Discord の不調で利用者の入力を失わせない)
#[utoipa::path(tag="attachments",params(EventPath),request_body=CoverInput,responses((status=200,body=CoverResult),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=403,body=ErrorBody),(status=404,body=ErrorBody)))]
#[put("/{guild_id}/{event_id}/attachments/cover")]
pub async fn set_cover(
    member: GuildMember,
    path: web::Path<EventPath>,
    input: web::Json<CoverInput>,
    state: web::Data<AppState>,
    storage: web::Data<AttachmentStorage>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let guild_id = member.guild_id();
    // 予定の更新 (Discord イベントの作り直しを含む) と直列にする。並ぶと、作り直す前の
    // イベントに画像を送ったり、作り直したイベントに画像が付かなかったりする
    let _event_lock = state.event_update_locks.lock(path.event_id).await;
    let mut tx = state.pool.begin().await?;
    attachments::lock_event(&mut tx, guild_id, path.event_id).await?;
    let recurring: bool =
        sqlx::query_scalar("SELECT series_id IS NOT NULL FROM events WHERE id=$1")
            .bind(path.event_id)
            .fetch_one(&mut *tx)
            .await?;
    if recurring && input.attachment_id.is_some() {
        return Err(ApiError::BadRequest(
            "繰り返し予定にはDiscordのイベントのカバーを設定できません".into(),
        ));
    }
    attachments::set_cover(
        &mut tx,
        guild_id,
        path.event_id,
        input.attachment_id.as_deref(),
    )
    .await?;
    tx.commit().await?;

    let status = match sync_cover(&state, &storage, guild_id, path.event_id).await {
        Ok(true) => CoverSync {
            status: CoverSyncStatus::Synced,
            error: None,
        },
        Ok(false) => CoverSync {
            status: CoverSyncStatus::NotLinked,
            error: None,
        },
        Err(err) => {
            tracing::warn!(
                guild_id,
                event_id = path.event_id,
                kind = err.kind(),
                "failed to sync a cover image to discord"
            );
            CoverSync {
                status: CoverSyncStatus::Failed,
                error: Some(ErrorBody {
                    error: err.kind(),
                    message: err.to_string(),
                }),
            }
        }
    };
    Ok(json(CoverResult {
        attachments: attachments::list(&state.pool, guild_id, path.event_id).await?,
        discord: status,
    }))
}

/// 保存済みのカバーを連携中の Discord イベントに反映する。連携していなければ `Ok(false)`
async fn sync_cover(
    state: &AppState,
    storage: &AttachmentStorage,
    guild_id: &str,
    event_id: i32,
) -> Result<bool, ApiError> {
    let Some(scheduled_event_id) = event_links::get(&state.pool, guild_id, event_id).await? else {
        return Ok(false);
    };
    let image = match attachments::find_cover(&state.pool, guild_id, event_id).await? {
        Some(row) => Some(storage.get()?.cover_data_uri(&row).await?),
        None => None,
    };
    let found = state
        .discord
        .set_scheduled_event_image(guild_id, &scheduled_event_id, image.as_deref())
        .await
        .map_err(describe_cover_error)?;
    if !found {
        return Err(ApiError::NotFound(
            "Discordのイベントが見つかりません。予定を保存し直すとイベントを作り直します".into(),
        ));
    }
    Ok(true)
}

fn describe_cover_error(err: DiscordError) -> ApiError {
    match &err {
        DiscordError::Status { status, .. } if *status == reqwest::StatusCode::BAD_REQUEST => {
            ApiError::BadRequest(
                "Discordがこの画像を受け付けませんでした。形式や大きさを見直してください".into(),
            )
        }
        _ => describe_scheduled_event_error(err),
    }
}
