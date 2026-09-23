//! サーバーごとの外部 ICS 購読 (#173)。登録・変更はサーバー管理権限で保護する。
use actix_web::{HttpResponse, delete, get, post, put, web};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

use super::{GuildMember, events::ListQuery};
use crate::{
    error::{ApiError, ErrorBody},
    external_calendars::{self, Calendar, CalendarView, ExternalEvent},
    models::now_jst,
    outbound_http,
    state::AppState,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CalendarInput {
    pub url: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExternalCalendarResult {
    pub calendar: CalendarView,
    pub events: Vec<ExternalEvent>,
}

fn require_manage(member: &GuildMember) -> Result<(), ApiError> {
    if member.permissions().can_manage_server() {
        Ok(())
    } else {
        Err(ApiError::Forbidden("manage permission is required".into()))
    }
}

fn validate(input: &CalendarInput) -> Result<(), ApiError> {
    outbound_http::parse_url(&input.url).map_err(|e| ApiError::BadRequest(e.into()))?;
    if input.name.trim().is_empty() || input.name.chars().count() > 32 {
        return Err(ApiError::BadRequest(
            "表示名は 1〜32 文字にしてください".into(),
        ));
    }
    if input.color.len() != 7
        || !input.color.starts_with('#')
        || !input.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(ApiError::BadRequest(
            "色は #RRGGBB 形式にしてください".into(),
        ));
    }
    Ok(())
}

async fn rows(pool: &PgPool, guild_id: &str) -> Result<Vec<Calendar>, ApiError> {
    Ok(sqlx::query_as::<_, Calendar>(
        "SELECT * FROM guild_external_calendars WHERE guild_id=$1 ORDER BY id",
    )
    .bind(guild_id)
    .fetch_all(pool)
    .await?)
}

async fn row(pool: &PgPool, guild_id: &str, id: i64) -> Result<Calendar, ApiError> {
    sqlx::query_as::<_, Calendar>(
        "SELECT * FROM guild_external_calendars WHERE guild_id=$1 AND id=$2",
    )
    .bind(guild_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| ApiError::NotFound("外部カレンダーが見つかりません".into()))
}

/// メンバー全員に表示名・色・取得状態を返す。限定公開 URL は管理権限がある人だけに返す。
#[utoipa::path(tag="events", responses((status=200, body=Vec<CalendarView>), (status=403, body=ErrorBody)))]
#[get("/{guild_id}/external-calendars")]
pub async fn list(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<CalendarView>>, ApiError> {
    let can_manage = member.permissions().can_manage_server();
    Ok(web::Json(
        rows(&state.pool, member.guild_id())
            .await?
            .iter()
            .map(|c| c.view(can_manage))
            .collect(),
    ))
}

/// 最大 5 件。ギルド単位の DB ロックで同時登録でも上限を守る。
#[utoipa::path(tag="events", request_body=CalendarInput, responses((status=201, body=CalendarView), (status=400, body=ErrorBody), (status=403, body=ErrorBody)))]
#[post("/{guild_id}/external-calendars")]
pub async fn create(
    member: GuildMember,
    body: web::Json<CalendarInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    validate(&body)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(member.guild_id())
        .execute(&mut *tx)
        .await?;
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM guild_external_calendars WHERE guild_id=$1")
            .bind(member.guild_id())
            .fetch_one(&mut *tx)
            .await?;
    if count >= external_calendars::MAX_CALENDARS {
        return Err(ApiError::BadRequest(
            "外部カレンダーは 5 件まで登録できます".into(),
        ));
    }
    if sqlx::query_scalar::<_, i64>(
        "SELECT id FROM guild_external_calendars WHERE guild_id=$1 AND url=$2",
    )
    .bind(member.guild_id())
    .bind(&body.url)
    .fetch_optional(&mut *tx)
    .await?
    .is_some()
    {
        return Err(ApiError::BadRequest("この URL は登録済みです".into()));
    }
    let calendar = sqlx::query_as::<_, Calendar>("INSERT INTO guild_external_calendars (guild_id,url,name,color,created_by,created_at) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *")
        .bind(member.guild_id()).bind(&body.url).bind(body.name.trim()).bind(&body.color)
        .bind(&member.user.discord_user_id).bind(now_jst()).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(HttpResponse::Created().json(calendar.view(true)))
}

#[utoipa::path(tag="events", request_body=CalendarInput, responses((status=200, body=CalendarView), (status=400, body=ErrorBody), (status=403, body=ErrorBody)))]
#[put("/{guild_id}/external-calendars/{id}")]
pub async fn update(
    member: GuildMember,
    id: web::Path<(String, i64)>,
    body: web::Json<CalendarInput>,
    state: web::Data<AppState>,
) -> Result<web::Json<CalendarView>, ApiError> {
    require_manage(&member)?;
    validate(&body)?;
    let old = row(&state.pool, member.guild_id(), id.1).await?;
    if old.url != body.url
        && sqlx::query_scalar::<_, i64>(
            "SELECT id FROM guild_external_calendars WHERE guild_id=$1 AND url=$2 AND id<>$3",
        )
        .bind(member.guild_id())
        .bind(&body.url)
        .bind(id.1)
        .fetch_optional(&state.pool)
        .await?
        .is_some()
    {
        return Err(ApiError::BadRequest("この URL は登録済みです".into()));
    }
    let calendar = sqlx::query_as::<_, Calendar>("UPDATE guild_external_calendars SET url=$3,name=$4,color=$5,last_fetched_at=CASE WHEN url=$3 THEN last_fetched_at ELSE NULL END,last_error=CASE WHEN url=$3 THEN last_error ELSE NULL END,etag=CASE WHEN url=$3 THEN etag ELSE NULL END,last_modified=CASE WHEN url=$3 THEN last_modified ELSE NULL END WHERE guild_id=$1 AND id=$2 RETURNING *")
        .bind(member.guild_id()).bind(id.1).bind(&body.url).bind(body.name.trim()).bind(&body.color).fetch_one(&state.pool).await?;
    state.external_feeds.invalidate(&id.1).await;
    Ok(web::Json(calendar.view(true)))
}

#[utoipa::path(tag="events", responses((status=204), (status=403, body=ErrorBody), (status=404, body=ErrorBody)))]
#[delete("/{guild_id}/external-calendars/{id}")]
pub async fn remove(
    member: GuildMember,
    id: web::Path<(String, i64)>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let affected = sqlx::query("DELETE FROM guild_external_calendars WHERE guild_id=$1 AND id=$2")
        .bind(member.guild_id())
        .bind(id.1)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if affected == 0 {
        return Err(ApiError::NotFound("外部カレンダーが見つかりません".into()));
    }
    state.external_feeds.invalidate(&id.1).await;
    Ok(HttpResponse::NoContent().finish())
}

#[utoipa::path(tag="events", responses((status=200, body=CalendarView), (status=403, body=ErrorBody), (status=404, body=ErrorBody)))]
#[post("/{guild_id}/external-calendars/{id}/refresh")]
pub async fn refresh(
    member: GuildMember,
    id: web::Path<(String, i64)>,
    state: web::Data<AppState>,
) -> Result<web::Json<CalendarView>, ApiError> {
    require_manage(&member)?;
    let mut calendar = row(&state.pool, member.guild_id(), id.1).await?;
    external_calendars::load(&state, &mut calendar, true).await;
    Ok(web::Json(calendar.view(true)))
}

/// 表示範囲だけに展開した読み取り専用の予定。取得失敗は購読先ごとに返し、他は表示する。
#[utoipa::path(tag="events", params(ListQuery), responses((status=200, body=Vec<ExternalCalendarResult>), (status=400, body=ErrorBody), (status=403, body=ErrorBody)))]
#[get("/{guild_id}/external")]
pub async fn events(
    member: GuildMember,
    query: web::Query<ListQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<ExternalCalendarResult>>, ApiError> {
    query.validate()?;
    let can_manage = member.permissions().can_manage_server();
    let calendars = rows(&state.pool, member.guild_id()).await?;
    let (start, end) = (query.start, query.end);
    let results = join_all(calendars.into_iter().map(|mut calendar| {
        let state = state.clone();
        async move {
            let parsed = external_calendars::load(&state, &mut calendar, false).await;
            let events = parsed
                .map(|items| external_calendars::visible(&items, calendar.id, start, end))
                .unwrap_or_default();
            ExternalCalendarResult {
                calendar: calendar.view(can_manage),
                events,
            }
        }
    }))
    .await;
    Ok(web::Json(results))
}
