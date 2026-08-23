//! 管理コンソールの読み取り専用 SQL コンソール (`/admin/sql`、#36)。
//!
//! 実行の制約 (読み取り専用トランザクション・タイムアウト・行数上限・保護テーブル) は
//! [`crate::models::admin_sql`] にある。ここでは実行のたびに (成功・失敗・拒否のどれでも) SQL と結果の概要を
//! `admin_audit_logs` に `sql.select` として残し、その履歴を返す。

use actix_web::{get, post, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    admin::AdminUser,
    error::{ApiError, ErrorBody},
    models::{
        admin_audit::{self, AuditEntry, AuditLog},
        admin_sql::{self, MAX_SQL_CHARS, STATEMENT_TIMEOUT, SqlError, SqlResult},
    },
    state::AppState,
};

/// 監査ログ上の action。履歴 (`GET /sql/history`) もこれで絞る
pub const SQL_AUDIT_ACTION: &str = "sql.select";
/// 履歴として返す件数
pub const HISTORY_LIMIT: i64 = 20;

#[derive(Deserialize, ToSchema)]
pub struct SqlRequest {
    /// 実行する SQL (1 文。SELECT / WITH / VALUES / TABLE / EXPLAIN / SHOW のみ)
    #[schema(example = "SELECT guild_id, name FROM guilds ORDER BY name LIMIT 10")]
    pub sql: String,
}

/// 読み取り専用 SQL の実行。専用の DB ロール (`discalendar_sql_console`) + `BEGIN READ ONLY` +
/// `statement_timeout` (10 秒) + 500 行 / 4 MiB 上限。Better Auth の `account` / `session` / `verification` は
/// ロールに権限が無く、読む文は実行前にも拒否する。成功・失敗にかかわらず `admin_audit_logs` に残す
#[utoipa::path(
    tag = "admin",
    request_body = SqlRequest,
    responses(
        (status = 200, body = SqlResult),
        (status = 400, description = "実行できない文 / Postgres のエラー (メッセージにそのまま入る)", body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 503, description = "SQL コンソール用の DB ロールが使えない (README の手順で作成する)", body = ErrorBody),
    )
)]
#[post("/sql")]
pub async fn run_sql(
    admin: AdminUser,
    body: web::Json<SqlRequest>,
    state: web::Data<AppState>,
) -> Result<web::Json<SqlResult>, ApiError> {
    let sql = body.sql.trim();
    let outcome = admin_sql::execute(&state.pool, sql, STATEMENT_TIMEOUT).await;
    // 監査ログに残す SQL は上限までに切る (長すぎて拒否した場合のため)
    let logged_sql: String = sql.chars().take(MAX_SQL_CHARS).collect();
    let (detail, response) = match outcome {
        Ok(result) => (
            serde_json::json!({
                "sql": logged_sql,
                "row_count": result.row_count,
                "truncated": result.truncated,
                "duration_ms": result.duration_ms,
            }),
            Ok(result),
        ),
        Err(SqlError::Rejected(message)) => (
            serde_json::json!({ "sql": logged_sql, "error": message, "rejected": true }),
            Err(ApiError::BadRequest(message)),
        ),
        Err(SqlError::Query(message)) => (
            serde_json::json!({ "sql": logged_sql, "error": message }),
            Err(ApiError::BadRequest(message)),
        ),
        // DB ロールの不備 (設定ミス)。実行していないが、試みたことは残す
        Err(SqlError::Unavailable(message)) => {
            tracing::error!(%message, "SQL console is unavailable");
            (
                serde_json::json!({ "sql": logged_sql, "error": message, "rejected": true }),
                Err(ApiError::Unavailable(message)),
            )
        }
        // 接続エラーなど。監査ログも書けない可能性が高いのでそのまま 500
        Err(SqlError::Other(error)) => return Err(error.into()),
    };
    admin_audit::record(
        &state.pool,
        &admin,
        AuditEntry {
            action: SQL_AUDIT_ACTION,
            detail: Some(detail),
            ..Default::default()
        },
    )
    .await?;
    tracing::info!(admin = %admin.discord_user_id, ok = response.is_ok(), "admin SQL executed");
    response.map(web::Json)
}

/// SQL コンソールの実行履歴 1 件 (監査ログの `sql.select` から組み立てる)
#[derive(Serialize, ToSchema)]
pub struct SqlHistoryEntry {
    pub id: i64,
    #[schema(example = "123456789012345678")]
    pub actor_discord_user_id: String,
    pub sql: String,
    /// 成功時の行数
    pub row_count: Option<u64>,
    pub truncated: Option<bool>,
    pub duration_ms: Option<u64>,
    /// 失敗・拒否時のメッセージ
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<AuditLog> for SqlHistoryEntry {
    fn from(log: AuditLog) -> Self {
        let detail = log.detail.unwrap_or(serde_json::Value::Null);
        let str_field = |key: &str| detail.get(key).and_then(|v| v.as_str()).map(str::to_owned);
        Self {
            id: log.id,
            actor_discord_user_id: log.actor_discord_user_id,
            sql: str_field("sql").unwrap_or_default(),
            row_count: detail.get("row_count").and_then(|v| v.as_u64()),
            truncated: detail.get("truncated").and_then(|v| v.as_bool()),
            duration_ms: detail.get("duration_ms").and_then(|v| v.as_u64()),
            error: str_field("error"),
            created_at: log.created_at,
        }
    }
}

/// 直近の実行履歴 (全管理者分、新しい順に 20 件)
#[utoipa::path(
    tag = "admin",
    responses(
        (status = 200, body = Vec<SqlHistoryEntry>),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/sql/history")]
pub async fn history(
    _admin: AdminUser,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<SqlHistoryEntry>>, ApiError> {
    let logs =
        admin_audit::list_recent_by_action(&state.pool, SQL_AUDIT_ACTION, HISTORY_LIMIT).await?;
    Ok(web::Json(
        logs.into_iter().map(SqlHistoryEntry::from).collect(),
    ))
}
