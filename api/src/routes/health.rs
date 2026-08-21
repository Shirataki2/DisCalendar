use actix_web::{Responder, get, web};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    error::{ApiError, ErrorBody},
    state::AppState,
};

/// バージョン文字列
#[utoipa::path(tag = "meta", responses((status = 200, body = String)))]
#[get("/")]
pub async fn index() -> impl Responder {
    format!("DisCalendar API v{}", env!("CARGO_PKG_VERSION"))
}

#[derive(Serialize, ToSchema)]
pub struct Health {
    #[schema(example = "ok")]
    pub status: &'static str,
}

/// DB への疎通を含むヘルスチェック
#[utoipa::path(
    tag = "meta",
    responses((status = 200, body = Health), (status = 500, body = ErrorBody))
)]
#[get("/healthz")]
pub async fn healthz(state: web::Data<AppState>) -> Result<web::Json<Health>, ApiError> {
    sqlx::query("SELECT 1").execute(&state.pool).await?;
    Ok(web::Json(Health { status: "ok" }))
}
