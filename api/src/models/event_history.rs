//! 予定の変更履歴 (`event_history` テーブル、#165) の読み取り。
//! 書き込みは api / bot 共通の [`crate::event_history`] で行う。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

/// 予定の詳細で一度に返す件数
pub const LIST_LIMIT: i64 = 50;

/// 履歴 1 件。`before` / `after` は予定のスナップショット
/// (name, description, location, color, is_all_day, start_at, end_at, notifications,
/// notification_mentions, discord_linked。日時は JST naive)。create は before が null、delete は after が null
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct EventHistoryEntry {
    pub id: i64,
    pub event_id: i32,
    /// 操作した Discord ユーザー。利用者以外の操作なら null
    #[schema(example = "123456789012345678")]
    pub actor_discord_user_id: Option<String>,
    /// web / bot / admin / mcp
    #[schema(example = "web")]
    pub source: String,
    /// create / update / delete / restore
    #[schema(example = "update")]
    pub action: String,
    #[schema(value_type = Option<Object>)]
    pub before: Option<serde_json::Value>,
    #[schema(value_type = Option<Object>)]
    pub after: Option<serde_json::Value>,
    /// 操作した時刻 (UTC。表示の変換は web)
    pub created_at: DateTime<Utc>,
}

/// ギルドに属する予定の履歴を新しい順に返す (他ギルドの予定 ID を指定しても返さない)
pub async fn list(
    pool: &PgPool,
    guild_id: &str,
    event_id: i32,
) -> sqlx::Result<Vec<EventHistoryEntry>> {
    sqlx::query_as(
        "SELECT id, event_id, actor_discord_user_id, source, action, before, after, created_at
         FROM event_history
         WHERE guild_id = $1 AND event_id = $2
         ORDER BY id DESC
         LIMIT $3",
    )
    .bind(guild_id)
    .bind(event_id)
    .bind(LIST_LIMIT)
    .fetch_all(pool)
    .await
}
