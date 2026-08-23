//! 管理コンソールの読み取り専用 SQL コンソール (`/admin/sql`、#36)。
//!
//! 実行の制約 (読み取り専用トランザクション・タイムアウト・行数上限・保護テーブル) は
//! [`crate::models::admin_sql`] にある。ここでは実行のたびに (成功・失敗・拒否のどれでも) SQL と結果の概要を
//! `admin_audit_logs` に `sql.select` として残し、その履歴を返す。

use std::{collections::HashSet, sync::Arc, time::Instant};

use actix_web::{get, post, web};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    admin::AdminUser,
    error::{ApiError, ErrorBody},
    models::{
        admin_audit::{self, AuditEntry, AuditLog},
        admin_sql::{
            self, KNOWN_WORDS_TTL, STATEMENT_TIMEOUT, SqlError, SqlResult,
            sanitize_error_for_audit, sanitize_sql_for_audit,
        },
    },
    state::{AppState, KnownWords},
};

/// 監査用の伏せ字で残してよい既知の単語。`KNOWN_WORDS_TTL` の間キャッシュし、取れなければ
/// 空集合 (= 引用符の無い識別子をすべて伏せる) にして安全側に倒す
async fn known_words(state: &AppState) -> Arc<HashSet<String>> {
    let mut cache = state.sql_known_words.lock().await;
    if let Some(cached) = cache.as_ref()
        && cached.loaded_at.elapsed() < KNOWN_WORDS_TTL
    {
        return Arc::clone(&cached.words);
    }
    match admin_sql::load_known_words(&state.pool).await {
        Ok(words) => {
            let words = Arc::new(words);
            *cache = Some(KnownWords {
                words: Arc::clone(&words),
                loaded_at: Instant::now(),
            });
            words
        }
        Err(error) => {
            tracing::warn!(error = %error, "failed to load known words for SQL audit redaction; redacting all identifiers");
            cache
                .as_ref()
                .map(|c| Arc::clone(&c.words))
                .unwrap_or_default()
        }
    }
}

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

/// 読み取り専用 SQL の実行。権限を絞った DB ロール (`discalendar_sql_console`) でログインした専用の接続 +
/// `BEGIN READ ONLY` + 10 秒の締切 + 500 行 / 4 MiB 上限。Better Auth の `account` / `session` / `verification` は
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
    let outcome = admin_sql::execute(&state.sql_console_pool, sql, STATEMENT_TIMEOUT).await;
    // 監査ログに残す SQL は文字列リテラルとコメントを伏せ (貼り付けた秘密値を残さない)、NUL を除き、
    // 上限までに切る (長すぎて拒否した場合のため)。エラーメッセージも Postgres が埋め込む値 ("...") を伏せる
    let known_words = known_words(&state).await;
    let logged_sql = sanitize_sql_for_audit(sql, |word| known_words.contains(word));
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
            serde_json::json!({
                "sql": logged_sql,
                "error": sanitize_error_for_audit(&message),
                "rejected": true,
            }),
            Err(ApiError::BadRequest(message)),
        ),
        Err(SqlError::Query(message)) => (
            serde_json::json!({ "sql": logged_sql, "error": sanitize_error_for_audit(&message) }),
            Err(ApiError::BadRequest(message)),
        ),
        // DB ロールの不備 (設定ミス)。実行していないが、試みたことは残す
        Err(SqlError::Unavailable(message)) => {
            tracing::error!(%message, "SQL console is unavailable");
            (
                serde_json::json!({
                    "sql": logged_sql,
                    "error": sanitize_error_for_audit(&message),
                    "rejected": true,
                }),
                Err(ApiError::Unavailable(message)),
            )
        }
        // 接続エラーなど (自分のバックエンドを pg_terminate_backend した場合も含む)。
        // 監査ログは別のプール (api のロール) に書けるので、記録してから 500
        Err(SqlError::Other(error)) => {
            tracing::error!(error = %error, "admin SQL failed with a connection/internal error");
            (
                serde_json::json!({
                    "sql": logged_sql,
                    "error": sanitize_error_for_audit(&format!("internal error: {error}")),
                }),
                Err(ApiError::Database(error)),
            )
        }
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

/// SQL コンソールの実行履歴 1 件 (監査ログの `sql.select` から組み立てる)。
/// `sql` は文字列リテラルとコメントを伏せたもの (`redact_sql`)
#[derive(Serialize, ToSchema)]
pub struct SqlHistoryEntry {
    pub id: i64,
    #[schema(example = "123456789012345678")]
    pub actor_discord_user_id: String,
    /// 実行した SQL (文字列リテラルは `'…'`、コメントは除いてある)
    pub sql: String,
    /// 成功時の行数
    pub row_count: Option<u64>,
    pub truncated: Option<bool>,
    pub duration_ms: Option<u64>,
    /// 失敗・拒否時のメッセージ (Postgres が埋め込む値は `"…"` に伏せてある)
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
