//! 管理コンソールの監査ログ (`admin_audit_logs` テーブル、#34 で追加)。
//!
//! `/admin/*` で行った書き込み操作 (予定の編集・削除、設定変更、定型操作) と SQL 実行は
//! すべて [`record`] を通して残す。閲覧 API は #37 で足す。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use crate::auth::AuthUser;

/// 記録する 1 件分の内容。`action` はドットつなぎの定数 (`"event.update"` など) にし、
/// 画面で絞り込めるようにする
#[derive(Debug, Clone, Default)]
pub struct AuditEntry<'a> {
    /// 操作の種類 (例: `event.update`, `guild_config.update`, `sql.select`)
    pub action: &'a str,
    /// 操作対象の種類 (例: `event`, `guild`)。対象がなければ `None`
    pub target_type: Option<&'a str>,
    /// 操作対象の ID (events.id や guild_id。Snowflake も文字列)
    pub target_id: Option<&'a str>,
    /// 変更前のスナップショット
    pub before: Option<serde_json::Value>,
    /// 変更後のスナップショット
    pub after: Option<serde_json::Value>,
    /// 付随情報 (実行した SQL と行数など)
    pub detail: Option<serde_json::Value>,
}

/// 保存済みの監査ログ 1 件
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AuditLog {
    pub id: i64,
    pub actor_user_id: String,
    #[schema(example = "123456789012345678")]
    pub actor_discord_user_id: String,
    #[schema(example = "event.update")]
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub before: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub after: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub detail: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// 監査ログを 1 件書く。操作本体と同じトランザクションで呼べるよう executor を受け取る
/// (操作が失敗したらログも残らない / ログが書けなければ操作も失敗する)
pub async fn record<'e>(
    executor: impl PgExecutor<'e>,
    actor: &AuthUser,
    entry: AuditEntry<'_>,
) -> sqlx::Result<AuditLog> {
    sqlx::query_as!(
        AuditLog,
        r#"
        INSERT INTO admin_audit_logs
            (actor_user_id, actor_discord_user_id, action, target_type, target_id, before, after, detail)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, actor_user_id, actor_discord_user_id, action, target_type, target_id,
                  before, after, detail, created_at
        "#,
        actor.id,
        actor.discord_user_id,
        entry.action,
        entry.target_type,
        entry.target_id,
        entry.before,
        entry.after,
        entry.detail,
    )
    .fetch_one(executor)
    .await
}
