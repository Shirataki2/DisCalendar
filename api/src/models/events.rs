use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgExecutor, PgPool};
use utoipa::ToSchema;

use super::notifications::Notification;
use crate::error::ApiError;

/// タイトルの最大文字数 (旧 Web のフォームと同じ)
pub const NAME_MAX_CHARS: usize = 32;
pub const DESCRIPTION_MAX_CHARS: usize = 1000;
pub const NOTIFICATIONS_MAX: usize = 10;

/// `events` テーブルの行
#[derive(Debug)]
pub struct EventRow {
    pub id: i32,
    pub guild_id: String,
    pub name: String,
    pub description: Option<String>,
    /// DB に入っている JSONB そのまま (`Notification::decode_all` で読む)
    pub notifications: Value,
    pub color: String,
    pub is_all_day: bool,
    pub start_at: NaiveDateTime,
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

/// API レスポンスの予定。日時はタイムゾーンなしの JST (`YYYY-MM-DDTHH:MM:SS`)
#[derive(Debug, Serialize, ToSchema)]
pub struct Event {
    #[schema(example = 1)]
    pub id: i32,
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    #[schema(example = "定例ミーティング")]
    pub name: String,
    pub description: Option<String>,
    pub notifications: Vec<Notification>,
    #[schema(example = "#2196F3")]
    pub color: String,
    pub is_all_day: bool,
    #[schema(example = "2026-08-22T10:00:00")]
    pub start_at: NaiveDateTime,
    #[schema(example = "2026-08-22T11:00:00")]
    pub end_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
}

impl From<EventRow> for Event {
    fn from(row: EventRow) -> Self {
        Self {
            id: row.id,
            guild_id: row.guild_id,
            name: row.name,
            description: row.description,
            notifications: Notification::decode_all(&row.notifications),
            color: row.color,
            is_all_day: row.is_all_day,
            start_at: row.start_at,
            end_at: row.end_at,
            created_at: row.created_at,
        }
    }
}

/// 予定の作成・更新リクエスト
#[derive(Debug, Deserialize, ToSchema)]
pub struct EventInput {
    #[schema(example = "定例ミーティング", max_length = 32)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub notifications: Vec<Notification>,
    /// `#RRGGBB`
    #[schema(example = "#2196F3")]
    pub color: String,
    #[serde(default)]
    pub is_all_day: bool,
    /// タイムゾーンなしの JST
    #[schema(example = "2026-08-22T10:00:00")]
    pub start_at: NaiveDateTime,
    #[schema(example = "2026-08-22T11:00:00")]
    pub end_at: NaiveDateTime,
}

impl EventInput {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.name.trim().is_empty() {
            return Err(ApiError::BadRequest("name is required".into()));
        }
        if self.name.chars().count() > NAME_MAX_CHARS {
            return Err(ApiError::BadRequest(format!(
                "name must be at most {NAME_MAX_CHARS} characters"
            )));
        }
        if let Some(description) = &self.description
            && description.chars().count() > DESCRIPTION_MAX_CHARS
        {
            return Err(ApiError::BadRequest(format!(
                "description must be at most {DESCRIPTION_MAX_CHARS} characters"
            )));
        }
        if !is_hex_color(&self.color) {
            return Err(ApiError::BadRequest(
                "color must be in #RRGGBB format".into(),
            ));
        }
        if self.end_at < self.start_at {
            return Err(ApiError::BadRequest(
                "end_at must not be before start_at".into(),
            ));
        }
        if self.notifications.len() > NOTIFICATIONS_MAX {
            return Err(ApiError::BadRequest(format!(
                "at most {NOTIFICATIONS_MAX} notifications are allowed"
            )));
        }
        Ok(())
    }
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// 期間 `[start, end)` に重なる予定 (途中から始まっている複数日の予定も含む)
pub async fn list_between(
    pool: &PgPool,
    guild_id: &str,
    start: NaiveDateTime,
    end: NaiveDateTime,
) -> sqlx::Result<Vec<EventRow>> {
    sqlx::query_as!(
        EventRow,
        r#"
        SELECT id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        FROM events
        WHERE guild_id = $1 AND start_at < $3 AND end_at >= $2
        ORDER BY start_at, id
        "#,
        guild_id,
        start,
        end
    )
    .fetch_all(pool)
    .await
}

pub async fn create<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    input: &EventInput,
    created_at: NaiveDateTime,
) -> sqlx::Result<EventRow> {
    let notifications = Notification::encode_all(&input.notifications);
    sqlx::query_as!(
        EventRow,
        r#"
        INSERT INTO events (guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        "#,
        guild_id,
        input.name,
        input.description,
        notifications,
        input.color,
        input.is_all_day,
        input.start_at,
        input.end_at,
        created_at
    )
    .fetch_one(executor)
    .await
}

/// ギルドに属する予定を 1 件取得し、トランザクションの終わりまで行をロックする (`FOR UPDATE`)。
/// 管理コンソールが監査ログの「変更前」として読むのに使う。更新・削除までの間に別トランザクション
/// (通常 API や Bot) が同じ行を書き換えて、ログの before と実際の直前の値がずれるのを防ぐ。
/// 他ギルドの ID を指定しても返さない。該当なしなら `None`
pub async fn find_by_id_for_update<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    id: i32,
) -> sqlx::Result<Option<EventRow>> {
    sqlx::query_as!(
        EventRow,
        r#"
        SELECT id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        FROM events
        WHERE id = $1 AND guild_id = $2
        FOR UPDATE
        "#,
        id,
        guild_id
    )
    .fetch_optional(executor)
    .await
}

/// ギルドに属する予定だけを更新する (他ギルドの ID を指定しても更新されない)。該当なしなら `None`。
/// 書き込み関数は executor を受け取るので、管理コンソールからは監査ログと同じトランザクションで呼べる
pub async fn update<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    id: i32,
    input: &EventInput,
) -> sqlx::Result<Option<EventRow>> {
    let notifications = Notification::encode_all(&input.notifications);
    sqlx::query_as!(
        EventRow,
        r#"
        UPDATE events
        SET name = $3, description = $4, notifications = $5, color = $6, is_all_day = $7, start_at = $8, end_at = $9
        WHERE id = $1 AND guild_id = $2
        RETURNING id, guild_id, name, description, notifications, color, is_all_day, start_at, end_at, created_at
        "#,
        id,
        guild_id,
        input.name,
        input.description,
        notifications,
        input.color,
        input.is_all_day,
        input.start_at,
        input.end_at
    )
    .fetch_optional(executor)
    .await
}

/// 削除できたら `true`
pub async fn delete<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
    id: i32,
) -> sqlx::Result<bool> {
    let result = sqlx::query!(
        "DELETE FROM events WHERE id = $1 AND guild_id = $2",
        id,
        guild_id
    )
    .execute(executor)
    .await?;
    Ok(result.rows_affected() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::notifications::NotificationUnit;

    fn input() -> EventInput {
        EventInput {
            name: "test".into(),
            description: None,
            notifications: vec![],
            color: "#2196F3".into(),
            is_all_day: false,
            start_at: "2026-08-22T10:00:00".parse().unwrap(),
            end_at: "2026-08-22T11:00:00".parse().unwrap(),
        }
    }

    #[test]
    fn valid_input_passes() {
        assert!(input().validate().is_ok());
    }

    #[test]
    fn rejects_blank_name() {
        let mut i = input();
        i.name = "   ".into();
        assert!(matches!(i.validate(), Err(ApiError::BadRequest(_))));
    }

    #[test]
    fn counts_name_length_in_chars() {
        let mut i = input();
        i.name = "あ".repeat(NAME_MAX_CHARS);
        assert!(i.validate().is_ok());
        i.name = "あ".repeat(NAME_MAX_CHARS + 1);
        assert!(i.validate().is_err());
    }

    #[test]
    fn rejects_bad_color() {
        for color in ["2196F3", "#2196F", "#GGGGGG", "#2196F3FF"] {
            let mut i = input();
            i.color = color.into();
            assert!(i.validate().is_err(), "{color}");
        }
    }

    #[test]
    fn rejects_end_before_start() {
        let mut i = input();
        i.end_at = "2026-08-22T09:59:59".parse().unwrap();
        assert!(i.validate().is_err());
        // 同時刻は許可 (終日予定など)
        i.end_at = i.start_at;
        assert!(i.validate().is_ok());
    }

    #[test]
    fn rejects_too_many_notifications() {
        let mut i = input();
        i.notifications = vec![
            Notification {
                num: 1,
                unit: NotificationUnit::Minutes
            };
            NOTIFICATIONS_MAX + 1
        ];
        assert!(i.validate().is_err());
    }
}
