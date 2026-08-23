//! 管理コンソールの監査ログ (`admin_audit_logs` テーブル、#34 で追加)。
//!
//! `/admin/*` で行った書き込み操作 (予定の編集・削除、設定変更、定型操作) と SQL 実行は
//! すべて [`record`] を通して残す。SQL コンソールの履歴は [`list_recent_by_action`]、
//! 監査ログ画面 (#37) の一覧は [`list`] で読む。

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

/// 監査ログ一覧の 1 ページあたりの件数
pub const PAGE_SIZE: i64 = 50;
/// ページ番号の上限 (`admin_guilds::MAX_PAGE` と同じ理由)
pub const MAX_PAGE: i64 = 1_000_000;
/// 絞り込み用に返す `action` の種類の上限
pub const ACTIONS_LIMIT: i64 = 100;

/// 監査ログの一覧 (新しい順)。`action` / `actor` (Discord ユーザー ID) が空なら絞り込まない
pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    action: &str,
    actor: &str,
    page: i64,
) -> sqlx::Result<Vec<AuditLog>> {
    let offset = (page.clamp(1, MAX_PAGE) - 1) * PAGE_SIZE;
    sqlx::query_as!(
        AuditLog,
        r#"
        SELECT id, actor_user_id, actor_discord_user_id, action, target_type, target_id,
               before, after, detail, created_at
        FROM admin_audit_logs
        WHERE ($1::text = '' OR action = $1)
          AND ($2::text = '' OR actor_discord_user_id = $2)
        ORDER BY id DESC
        LIMIT $3 OFFSET $4
        "#,
        action,
        actor,
        PAGE_SIZE,
        offset,
    )
    .fetch_all(executor)
    .await
}

/// [`list`] と同じ条件での総件数
pub async fn count<'e>(
    executor: impl PgExecutor<'e>,
    action: &str,
    actor: &str,
) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) AS "count!"
        FROM admin_audit_logs
        WHERE ($1::text = '' OR action = $1)
          AND ($2::text = '' OR actor_discord_user_id = $2)
        "#,
        action,
        actor,
    )
    .fetch_one(executor)
    .await
}

/// 記録されている `action` の種類 (画面の絞り込み用)。
/// 監査ログは管理者の操作しか入らないので行数は少なく、全表走査でも問題にならない
pub async fn actions<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar!(
        r#"SELECT DISTINCT action AS "action!" FROM admin_audit_logs ORDER BY action LIMIT $1"#,
        ACTIONS_LIMIT
    )
    .fetch_all(executor)
    .await
}

/// 指定した `action` の監査ログを新しい順に `limit` 件返す (SQL コンソールの実行履歴用)
pub async fn list_recent_by_action<'e>(
    executor: impl PgExecutor<'e>,
    action: &str,
    limit: i64,
) -> sqlx::Result<Vec<AuditLog>> {
    sqlx::query_as!(
        AuditLog,
        r#"
        SELECT id, actor_user_id, actor_discord_user_id, action, target_type, target_id,
               before, after, detail, created_at
        FROM admin_audit_logs
        WHERE action = $1
        ORDER BY id DESC
        LIMIT $2
        "#,
        action,
        limit,
    )
    .fetch_all(executor)
    .await
}
