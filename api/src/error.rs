use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;
use utoipa::ToSchema;

use crate::discord::DiscordError;

/// ハンドラが返すエラー。`ResponseError` で JSON のエラーレスポンスに変換される
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("authentication required")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("Discord API is rate limited, retry later")]
    RateLimited,
    #[error("failed to call Discord API")]
    Discord(#[source] DiscordError),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("internal server error")]
    Internal(#[from] anyhow::Error),
}

/// エラーレスポンスのボディ
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    /// 機械可読なエラー種別 (`unauthorized`, `forbidden`, `not_found`, `bad_request`, ...)
    #[schema(example = "not_found")]
    pub error: &'static str,
    /// 人間向けのメッセージ
    #[schema(example = "event not found")]
    pub message: String,
}

impl ApiError {
    fn kind(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::NotFound(_) => "not_found",
            Self::BadRequest(_) => "bad_request",
            Self::RateLimited => "rate_limited",
            Self::Discord(_) => "discord_error",
            Self::Database(_) => "database_error",
            Self::Internal(_) => "internal_error",
        }
    }
}

impl From<DiscordError> for ApiError {
    fn from(err: DiscordError) -> Self {
        match err {
            DiscordError::RateLimited => Self::RateLimited,
            other => Self::Discord(other),
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::RateLimited => StatusCode::SERVICE_UNAVAILABLE,
            Self::Discord(_) => StatusCode::BAD_GATEWAY,
            Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        // 5xx 系は原因をログに残す (レスポンスには内部情報を出さない)
        match self {
            Self::Discord(e) => tracing::error!(error = %e, "discord api error"),
            Self::Database(e) => tracing::error!(error = %e, "database error"),
            Self::Internal(e) => tracing::error!(error = ?e, "internal error"),
            _ => {}
        }
        HttpResponse::build(self.status_code()).json(ErrorBody {
            error: self.kind(),
            message: self.to_string(),
        })
    }
}
