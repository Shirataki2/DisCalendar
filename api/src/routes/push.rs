//! 購読情報はセッション本人だけが操作する。URL と鍵は一覧・ログに出さない。
use crate::{
    auth::AuthUser,
    error::{ApiError, ErrorBody},
    models::push::{self, PushScope, PushSettings, SubscriptionInput},
    state::AppState,
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Deserialize;
use utoipa::ToSchema;

#[utoipa::path(tag = "push", responses((status = 200, body = PushSettings), (status = 401, body = ErrorBody)))]
#[get("/users/@me/push-subscriptions")]
pub async fn list(
    user: AuthUser,
    state: web::Data<AppState>,
) -> Result<web::Json<PushSettings>, ApiError> {
    Ok(web::Json(push::get(&state.pool, &user.id).await?))
}
#[utoipa::path(tag = "push", request_body = SubscriptionInput, responses((status = 204), (status = 400, body = ErrorBody), (status = 401, body = ErrorBody)))]
#[post("/users/@me/push-subscriptions")]
pub async fn subscribe(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<SubscriptionInput>,
) -> Result<HttpResponse, ApiError> {
    push::subscribe(&state.pool, &user.id, &body).await?;
    Ok(HttpResponse::NoContent().finish())
}
#[derive(Deserialize, ToSchema)]
pub struct ScopeInput {
    pub scope: PushScope,
}
#[utoipa::path(tag = "push", request_body = ScopeInput, responses((status = 204), (status = 401, body = ErrorBody)))]
#[put("/users/@me/push-settings")]
pub async fn settings(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<ScopeInput>,
) -> Result<HttpResponse, ApiError> {
    push::set_scope(&state.pool, &user.id, &body.scope).await?;
    Ok(HttpResponse::NoContent().finish())
}
#[utoipa::path(tag = "push", params(("id" = i32, Path)), responses((status = 204), (status = 401, body = ErrorBody)))]
#[delete("/users/@me/push-subscriptions/{id}")]
pub async fn remove(
    user: AuthUser,
    state: web::Data<AppState>,
    id: web::Path<i32>,
) -> Result<HttpResponse, ApiError> {
    push::remove(&state.pool, &user.id, *id).await?;
    Ok(HttpResponse::NoContent().finish())
}
#[derive(Deserialize, ToSchema)]
pub struct EndpointInput {
    pub endpoint: String,
}
#[utoipa::path(tag = "push", request_body = EndpointInput, responses((status = 204), (status = 401, body = ErrorBody)))]
#[delete("/users/@me/push-subscriptions")]
pub async fn remove_current(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<EndpointInput>,
) -> Result<HttpResponse, ApiError> {
    push::remove_endpoint(&state.pool, &user.id, &body.endpoint).await?;
    Ok(HttpResponse::NoContent().finish())
}
