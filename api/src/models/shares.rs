//! 予定単位の共有リンク。公開用の型には作成者・通知・メンバー情報を含めない。
use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use super::feed_tokens;

#[derive(Debug, Serialize, ToSchema)]
pub struct ShareLink {
    pub token: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SharedEvent {
    pub guild_id: String,
    pub guild_name: String,
    pub guild_avatar_url: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
}

/// 既存リンクは保持する。INSERT の入力もギルドと予定 ID の両方で絞る。
pub async fn issue(
    pool: &PgPool,
    guild_id: &str,
    event_id: i32,
) -> sqlx::Result<Option<ShareLink>> {
    let token = feed_tokens::generate_token();
    sqlx::query_as!(
        ShareLink,
        r#"
        INSERT INTO event_share_links (event_id, token)
        SELECT id, $3 FROM events WHERE guild_id = $1 AND id = $2
        ON CONFLICT (event_id) DO UPDATE SET token = event_share_links.token
        RETURNING token
    "#,
        guild_id,
        event_id,
        token
    )
    .fetch_optional(pool)
    .await
}

pub async fn get(pool: &PgPool, guild_id: &str, event_id: i32) -> sqlx::Result<Option<ShareLink>> {
    sqlx::query_as!(
        ShareLink,
        r#"
        SELECT s.token FROM event_share_links s JOIN events e ON e.id = s.event_id
        WHERE e.guild_id = $1 AND e.id = $2
    "#,
        guild_id,
        event_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn revoke(pool: &PgPool, guild_id: &str, event_id: i32) -> sqlx::Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM event_share_links s USING events e
        WHERE s.event_id = e.id AND e.guild_id = $1 AND e.id = $2
    "#,
        guild_id,
        event_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Bot が退出したギルドは JOIN で除外する。編集内容は毎回現在の予定から取得する。
pub async fn find(pool: &PgPool, token: &str) -> sqlx::Result<Option<SharedEvent>> {
    sqlx::query_as!(
        SharedEvent,
        r#"
        SELECT e.guild_id, g.name AS guild_name, g.avatar_url AS guild_avatar_url,
               e.name, e.description, e.is_all_day, e.start_at, e.end_at
        FROM event_share_links s
        JOIN events e ON e.id = s.event_id
        JOIN guilds g ON g.guild_id = e.guild_id
        WHERE s.token = $1
    "#,
        token
    )
    .fetch_optional(pool)
    .await
}
