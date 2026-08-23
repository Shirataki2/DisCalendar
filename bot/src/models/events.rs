use chrono::NaiveDateTime;
use sqlx::PgPool;

use super::notifications::Notification;

/// タイトルの最大文字数 (api の `EventInput::validate` / web のフォームと同じ)
pub const NAME_MAX_CHARS: usize = 32;
pub const DESCRIPTION_MAX_CHARS: usize = 1000;

/// `events` テーブルの行。日時はタイムゾーンなしの JST
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: i32,
    pub guild_id: String,
    pub name: String,
    pub description: Option<String>,
    /// 旧形式の JSON 文字列 (`Notification::decode_all` で読む)
    pub notifications: Vec<String>,
    /// `#RRGGBB`
    pub color: String,
    /// 終日予定。`start_at` は開始日の 0:00、`end_at` は終了日 (含む) の 0:00 (web と同じ表現)
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

impl Event {
    pub fn notifications(&self) -> Vec<Notification> {
        Notification::decode_all(&self.notifications)
    }
}

/// 作成する予定
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewEvent<'a> {
    pub guild_id: &'a str,
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub notifications: &'a [Notification],
    pub color: &'a str,
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

pub async fn create(pool: &PgPool, event: &NewEvent<'_>) -> sqlx::Result<Event> {
    let notifications = Notification::encode_all(event.notifications);
    sqlx::query_as!(
        Event,
        r#"
        INSERT INTO events (guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        "#,
        event.guild_id,
        event.name,
        event.description,
        &notifications[..],
        event.color,
        event.is_all_day,
        event.start_at,
        event.end_at,
        event.created_at
    )
    .fetch_one(pool)
    .await
}

/// ギルドの全予定 (開始日時順)
pub async fn list_all(pool: &PgPool, guild_id: &str) -> sqlx::Result<Vec<Event>> {
    sqlx::query_as!(
        Event,
        r#"
        SELECT id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        FROM events WHERE guild_id = $1 ORDER BY start_at, id
        "#,
        guild_id
    )
    .fetch_all(pool)
    .await
}

/// `now` 以前に始まった予定 (旧 Bot の `find_past_events` と同じく `start_at <= now`)
pub async fn list_past(
    pool: &PgPool,
    guild_id: &str,
    now: NaiveDateTime,
) -> sqlx::Result<Vec<Event>> {
    sqlx::query_as!(
        Event,
        r#"
        SELECT id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        FROM events WHERE guild_id = $1 AND start_at <= $2 ORDER BY start_at, id
        "#,
        guild_id,
        now
    )
    .fetch_all(pool)
    .await
}

/// `now` 以降に始まる予定 (旧 Bot の `find_future_events` と同じく `start_at >= now`)
pub async fn list_future(
    pool: &PgPool,
    guild_id: &str,
    now: NaiveDateTime,
) -> sqlx::Result<Vec<Event>> {
    sqlx::query_as!(
        Event,
        r#"
        SELECT id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        FROM events WHERE guild_id = $1 AND start_at >= $2 ORDER BY start_at, id
        "#,
        guild_id,
        now
    )
    .fetch_all(pool)
    .await
}

/// `now` 以降に始まる予定を全ギルド横断で取得する
/// (旧 Bot の `find_all_future_events` と同じく `start_at >= now`。通知タスクが使う)
pub async fn list_all_future(pool: &PgPool, now: NaiveDateTime) -> sqlx::Result<Vec<Event>> {
    sqlx::query_as!(
        Event,
        r#"
        SELECT id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        FROM events WHERE start_at >= $1 ORDER BY start_at, id
        "#,
        now
    )
    .fetch_all(pool)
    .await
}
