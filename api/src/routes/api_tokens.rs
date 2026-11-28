//! サーバー用 API トークン (#405) の発行・一覧・ローテーション・失効・削除。
//! 閲覧・操作は restricted モードに関わらずサーバー管理権限 (`can_manage_server`) を要求する。

use actix_web::{HttpResponse, delete, get, post, web};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

use super::GuildMember;
use crate::{
    error::{ApiError, ErrorBody},
    models::api_tokens::{self, ApiTokenInfo, ApiTokenScope, IssuedApiToken, compute_expires_at},
    state::AppState,
};

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiTokenInput {
    #[schema(example = "GitHub Actions 連携", max_length = 64)]
    pub name: String,
    pub scope: ApiTokenScope,
    /// 有効期限の日数 (1〜3650)。省略または null で無期限
    #[schema(example = 90)]
    pub expires_in_days: Option<u32>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct TokenPath {
    /// ギルド ID
    #[allow(dead_code)]
    pub guild_id: String,
    /// API トークン ID
    pub token_id: i64,
}

fn require_manage(member: &GuildMember) -> Result<(), ApiError> {
    if member.permissions().can_manage_server() {
        Ok(())
    } else {
        Err(ApiError::Forbidden("サーバー管理権限が必要です".into()))
    }
}

fn no_store_json<T: serde::Serialize>(
    status: actix_web::http::StatusCode,
    body: T,
) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header(("Cache-Control", "no-store"))
        .json(body)
}

/// サーバーの API トークン一覧と直近の呼び出しログ
#[utoipa::path(
    tag = "api-tokens",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 200, body = Vec<ApiTokenInfo>),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/{guild_id}/api-tokens")]
pub async fn list(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let items = api_tokens::list_with_logs(&state.pool, member.guild_id()).await?;
    Ok(no_store_json(actix_web::http::StatusCode::OK, items))
}

/// 新しい API トークンを発行する (平文トークンはレスポンスに 1 回だけ含む)
#[utoipa::path(
    tag = "api-tokens",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    request_body = CreateApiTokenInput,
    responses(
        (status = 201, body = IssuedApiToken),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[post("/{guild_id}/api-tokens")]
pub async fn create(
    member: GuildMember,
    state: web::Data<AppState>,
    body: web::Json<CreateApiTokenInput>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let expires_at = compute_expires_at(body.expires_in_days)?;
    let issued = api_tokens::create(
        &state.pool,
        member.guild_id(),
        &body.name,
        body.scope,
        expires_at,
        &member.user.discord_user_id,
    )
    .await?;
    tracing::info!(
        guild_id = member.guild_id(),
        token_id = %issued.id,
        user_id = %member.user.discord_user_id,
        scope = issued.scope.as_str(),
        "api token created"
    );
    Ok(no_store_json(actix_web::http::StatusCode::CREATED, issued))
}

/// API トークンをローテーションする (旧トークンは即座に無効になり、新しい平文を 1 回だけ返す)
#[utoipa::path(
    tag = "api-tokens",
    params(TokenPath),
    responses(
        (status = 200, body = IssuedApiToken),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[post("/{guild_id}/api-tokens/{token_id}/rotate")]
pub async fn rotate(
    member: GuildMember,
    path: web::Path<TokenPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    let issued = api_tokens::rotate(&state.pool, member.guild_id(), path.token_id).await?;
    tracing::info!(
        guild_id = member.guild_id(),
        token_id = path.token_id,
        user_id = %member.user.discord_user_id,
        "api token rotated"
    );
    Ok(no_store_json(actix_web::http::StatusCode::OK, issued))
}

/// API トークンを失効させる
#[utoipa::path(
    tag = "api-tokens",
    params(TokenPath),
    responses(
        (status = 204),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[post("/{guild_id}/api-tokens/{token_id}/revoke")]
pub async fn revoke(
    member: GuildMember,
    path: web::Path<TokenPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    api_tokens::revoke(&state.pool, member.guild_id(), path.token_id).await?;
    tracing::info!(
        guild_id = member.guild_id(),
        token_id = path.token_id,
        user_id = %member.user.discord_user_id,
        "api token revoked"
    );
    Ok(HttpResponse::NoContent()
        .insert_header(("Cache-Control", "no-store"))
        .finish())
}

/// API トークンと呼び出しログを削除する
#[utoipa::path(
    tag = "api-tokens",
    params(TokenPath),
    responses(
        (status = 204),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, body = ErrorBody),
    )
)]
#[delete("/{guild_id}/api-tokens/{token_id}")]
pub async fn remove(
    member: GuildMember,
    path: web::Path<TokenPath>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    api_tokens::remove(&state.pool, member.guild_id(), path.token_id).await?;
    tracing::info!(
        guild_id = member.guild_id(),
        token_id = path.token_id,
        user_id = %member.user.discord_user_id,
        "api token deleted"
    );
    Ok(HttpResponse::NoContent()
        .insert_header(("Cache-Control", "no-store"))
        .finish())
}
