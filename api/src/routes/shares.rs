//! 共有リンクの発行・失効は予定の編集権限を要求する。公開取得だけは認証不要。
use super::{
    GuildMember,
    events::{EventPath, ensure_can_edit},
};
use crate::{
    error::{ApiError, ErrorBody},
    models::{
        events, feed_tokens,
        shares::{self, ShareLink, SharedEvent},
    },
    state::AppState,
};
use actix_web::{HttpResponse, delete, get, http::header, post, web};

async fn ensure_event(
    state: &AppState,
    member: &GuildMember,
    event_id: i32,
) -> Result<(), ApiError> {
    ensure_can_edit(&state.pool, member).await?;
    events::find_by_id(&state.pool, member.guild_id(), event_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    Ok(())
}

#[utoipa::path(tag = "shares", params(EventPath), responses((status = 200, body = Option<ShareLink>), (status = 403, body = ErrorBody)))]
#[get("/{guild_id}/{event_id}/share")]
pub async fn get_share(
    member: GuildMember,
    path: web::Path<EventPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_event(&state, &member, path.event_id).await?;
    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(shares::get(&state.pool, member.guild_id(), path.event_id).await?))
}

#[utoipa::path(tag = "shares", params(EventPath), responses((status = 200, body = ShareLink), (status = 403, body = ErrorBody), (status = 404, body = ErrorBody)))]
#[post("/{guild_id}/{event_id}/share")]
pub async fn issue_share(
    member: GuildMember,
    path: web::Path<EventPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_can_edit(&state.pool, &member).await?;
    let link = shares::issue(&state.pool, member.guild_id(), path.event_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("event not found".into()))?;
    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(link))
}

#[utoipa::path(tag = "shares", params(EventPath), responses((status = 204), (status = 403, body = ErrorBody)))]
#[delete("/{guild_id}/{event_id}/share")]
pub async fn revoke_share(
    member: GuildMember,
    path: web::Path<EventPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    ensure_event(&state, &member, path.event_id).await?;
    shares::revoke(&state.pool, member.guild_id(), path.event_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[utoipa::path(tag = "shares", params(("token" = String, Path)), responses((status = 200, body = SharedEvent), (status = 404, body = ErrorBody)), security(()))]
#[get("/share/{token}")]
pub async fn public_share(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let not_found = || ApiError::NotFound("share not found".into());
    if !feed_tokens::is_token(&path) {
        return Err(not_found());
    }
    let event = shares::find(&state.pool, &path)
        .await?
        .ok_or_else(not_found)?;
    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(event))
}
