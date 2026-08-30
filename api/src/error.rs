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
    /// **Bot** の権限が足りず Discord の操作ができない (#94)。利用者自身の権限不足 ([`Self::Forbidden`]) と
    /// 区別する: 直すには Bot の再招待が要るので、web は別の案内を出す。
    /// 権限のキャッシュ (最大 5 分) が古いと、UI で有効なまま保存時にここへ来ることがある
    #[error("{0}")]
    BotPermission(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    /// 同じ資源への並行更新とぶつかって完了できなかった (やり直せば通る)
    #[error("{0}")]
    Conflict(String),
    #[error("Discord API is rate limited, retry later")]
    RateLimited,
    /// 機能が設定不備などで使えない (SQL コンソール用の DB ロールが無い等)。メッセージは利用者に見せる
    #[error("{0}")]
    Unavailable(String),
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
    /// 機械可読なエラー種別 (`unauthorized`, `forbidden`, `not_found`, `bad_request`, `unavailable`, ...)
    #[schema(example = "not_found")]
    pub error: &'static str,
    /// 人間向けのメッセージ
    #[schema(example = "event not found")]
    pub message: String,
}

impl ApiError {
    /// 機械可読なエラー種別。レスポンスの `error` とリクエスト完了ログ (#110) の両方で使う
    /// (メッセージは Postgres が入力値を埋め込むことがあるので、ログにはこちらだけを載せる)
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::BotPermission(_) => "bot_permission",
            Self::NotFound(_) => "not_found",
            Self::BadRequest(_) => "bad_request",
            Self::Conflict(_) => "conflict",
            Self::RateLimited => "rate_limited",
            Self::Unavailable(_) => "unavailable",
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
            // 呼び出し元が検証済みの値だけを渡すので通常は起きないが、
            // 万一 URL に使えない ID が来ていたら入力の誤りとして返す (502 にはしない)
            DiscordError::InvalidId => Self::BadRequest("invalid discord id".into()),
            other => Self::Discord(other),
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) | Self::BotPermission(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::RateLimited | Self::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Discord(_) => StatusCode::BAD_GATEWAY,
            Self::Database(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        // 5xx 系は原因をログに残す (レスポンスには内部情報を出さない)。
        // ここが 5xx の唯一の ERROR ログ = Sentry のイベント 1 件で、リクエスト完了ログ (#110) は
        // INFO なのでイベントにならない。5xx を増やすときはここにも足す
        match self {
            Self::Discord(e) => tracing::error!(error = %e, "discord api error"),
            Self::Database(e) => tracing::error!(error = %e, "database error"),
            Self::Internal(e) => tracing::error!(error = ?e, "internal error"),
            // Discord クライアント側のログは WARN なので、503 を返すことはここで残す
            Self::RateLimited => tracing::error!("discord api is rate limited"),
            // Unavailable は唯一の生成元 (routes/admin_sql.rs) が理由付きで ERROR を出しているので、
            // ここで出すと同じ障害が二重にイベント化される
            _ => {}
        }
        HttpResponse::build(self.status_code()).json(ErrorBody {
            error: self.kind(),
            message: self.to_string(),
        })
    }
}
