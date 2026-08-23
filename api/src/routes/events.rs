use actix_web::{HttpResponse, delete, get, post, put, web};
use chrono::{Duration, NaiveDateTime};
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::IntoParams;

use super::GuildMember;
use crate::{
    error::{ApiError, ErrorBody},
    models::{
        events::{self, Event, EventInput},
        guilds, now_jst,
    },
    state::AppState,
};

/// 一度に取得できる期間の上限 (FullCalendar の月表示は最大 6 週間)
const MAX_RANGE_DAYS: i64 = 400;

#[derive(Deserialize, IntoParams)]
pub struct ListQuery {
    /// 取得範囲の開始 (JST、この時刻を含む)
    #[param(example = "2026-08-01T00:00:00")]
    pub start: NaiveDateTime,
    /// 取得範囲の終了 (JST、この時刻を含まない)
    #[param(example = "2026-09-01T00:00:00")]
    pub end: NaiveDateTime,
}

impl ListQuery {
    /// 範囲の向きと長さを確認する (管理コンソールの一覧でも同じ条件を使う)
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.end <= self.start {
            return Err(ApiError::BadRequest("end must be after start".into()));
        }
        if self.end - self.start > Duration::days(MAX_RANGE_DAYS) {
            return Err(ApiError::BadRequest(format!(
                "range must be at most {MAX_RANGE_DAYS} days"
            )));
        }
        Ok(())
    }
}

#[derive(Deserialize, IntoParams)]
pub struct EventPath {
    /// ギルド ID (認可は `GuildMember` が行うのでここでは読まない。OpenAPI 用)
    #[allow(dead_code)]
    pub guild_id: String,
    /// 予定 ID
    pub event_id: i32,
}

/// 期間に重なる予定の一覧
#[utoipa::path(
    tag = "events",
    params(("guild_id" = String, Path, description = "ギルド ID"), ListQuery),
    responses(
        (status = 200, body = Vec<Event>),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/{guild_id}")]
pub async fn list(
    member: GuildMember,
    query: web::Query<ListQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<Event>>, ApiError> {
    query.validate()?;
    let rows = events::list_between(&state.pool, member.guild_id(), query.start, query.end).await?;
    Ok(web::Json(rows.into_iter().map(Event::from).collect()))
}

/// 予定の作成
#[utoipa::path(
    tag = "events",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    request_body = EventInput,
    responses(
        (status = 201, body = Event),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / restricted モードで権限なし", body = ErrorBody),
    )
)]
#[post("/{guild_id}")]
pub async fn create(
    member: GuildMember,
    body: web::Json<EventInput>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    body.validate()?;
    let row = events::create(&state.pool, member.guild_id(), &body, now_jst()).await?;
    tracing::info!(guild_id = member.guild_id(), event_id = row.id, user_id = %member.user.discord_user_id, "event created");
    Ok(HttpResponse::Created().json(Event::from(row)))
}

/// 予定の更新 (全フィールド置き換え)
#[utoipa::path(
    tag = "events",
    params(EventPath),
    request_body = EventInput,
    responses(
        (status = 200, body = Event),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / restricted モードで権限なし", body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[put("/{guild_id}/{event_id}")]
pub async fn update(
    member: GuildMember,
    path: web::Path<EventPath>,
    body: web::Json<EventInput>,
    state: web::Data<AppState>,
) -> Result<web::Json<Event>, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    body.validate()?;
    let row = events::update(&state.pool, member.guild_id(), path.event_id, &body)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    tracing::info!(guild_id = member.guild_id(), event_id = row.id, user_id = %member.user.discord_user_id, "event updated");
    Ok(web::Json(Event::from(row)))
}

/// 予定の削除
#[utoipa::path(
    tag = "events",
    params(EventPath),
    responses(
        (status = 204),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / restricted モードで権限なし", body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[delete("/{guild_id}/{event_id}")]
pub async fn delete(
    member: GuildMember,
    path: web::Path<EventPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    if !events::delete(&state.pool, member.guild_id(), path.event_id).await? {
        return Err(ApiError::NotFound("event not found".into()));
    }
    tracing::info!(guild_id = member.guild_id(), event_id = path.event_id, user_id = %member.user.discord_user_id, "event deleted");
    Ok(HttpResponse::NoContent().finish())
}

/// restricted モードのギルドでは管理権限を持つユーザーだけが予定を編集できる。
/// 旧実装はこの判定をクライアント側だけで行っていたが、サーバー側で強制する
async fn ensure_can_edit(pool: &PgPool, member: &GuildMember) -> Result<(), ApiError> {
    let config = guilds::get_config(pool, member.guild_id()).await?;
    if config.restricted && !member.permissions().can_manage_server() {
        return Err(ApiError::Forbidden(
            "this guild restricts editing events to users with manage permissions".into(),
        ));
    }
    Ok(())
}
