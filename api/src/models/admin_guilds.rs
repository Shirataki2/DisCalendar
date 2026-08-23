//! 管理コンソール用のギルド一覧・詳細 (#35)。
//!
//! `guilds` に `guild_config.restricted`、`event_settings.channel_id`、予定数を付けて返す。
//! Bot の参加状況は DB には無いので routes 側で Discord API を引いて足す。

use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

/// 1 ページあたりの件数
pub const PAGE_SIZE: i64 = 50;

/// ギルド一覧・詳細の 1 行
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct AdminGuild {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    #[schema(example = "ja")]
    pub locale: String,
    /// `guild_config.restricted` (行が無ければ false)
    pub restricted: bool,
    /// `/init` で設定した通知先チャンネル (`event_settings`)。未設定なら null
    #[schema(example = "782502586817314820")]
    pub channel_id: Option<String>,
    /// 予定の総数
    #[schema(example = 12)]
    pub event_count: i64,
}

/// 名前の部分一致用に `ILIKE` のパターンを作る (`%` / `_` / `\` はリテラル扱い)
fn name_pattern(q: &str) -> String {
    let mut escaped = String::with_capacity(q.len() + 2);
    escaped.push('%');
    for c in q.chars() {
        if matches!(c, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped.push('%');
    escaped
}

/// 一覧 (検索・ページング)。`q` が空なら全件。guild_id の完全一致か名前の部分一致 (大文字小文字を区別しない)。
/// `page` は 1 始まり
pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    q: &str,
    page: i64,
) -> sqlx::Result<Vec<AdminGuild>> {
    let offset = (page.max(1) - 1) * PAGE_SIZE;
    sqlx::query_as!(
        AdminGuild,
        r#"
        SELECT g.guild_id, g.name, g.avatar_url, g.locale,
               COALESCE(c.restricted, false) AS "restricted!",
               (SELECT s.channel_id FROM event_settings s WHERE s.guild_id = g.guild_id ORDER BY s.id LIMIT 1) AS channel_id,
               (SELECT count(*) FROM events e WHERE e.guild_id = g.guild_id) AS "event_count!"
        FROM guilds g
        LEFT JOIN guild_config c ON c.guild_id = g.guild_id
        WHERE $1::text = '' OR g.guild_id = $1 OR g.name ILIKE $2
        ORDER BY g.name, g.id
        LIMIT $3 OFFSET $4
        "#,
        q,
        name_pattern(q),
        PAGE_SIZE,
        offset
    )
    .fetch_all(executor)
    .await
}

/// 一覧と同じ条件での総件数
pub async fn count<'e>(executor: impl PgExecutor<'e>, q: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar!(
        r#"
        SELECT count(*) AS "count!"
        FROM guilds g
        WHERE $1::text = '' OR g.guild_id = $1 OR g.name ILIKE $2
        "#,
        q,
        name_pattern(q)
    )
    .fetch_one(executor)
    .await
}

/// 詳細。`guilds` に行が無ければ `None` (Bot が一度も参加していない)
pub async fn find<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
) -> sqlx::Result<Option<AdminGuild>> {
    sqlx::query_as!(
        AdminGuild,
        r#"
        SELECT g.guild_id, g.name, g.avatar_url, g.locale,
               COALESCE(c.restricted, false) AS "restricted!",
               (SELECT s.channel_id FROM event_settings s WHERE s.guild_id = g.guild_id ORDER BY s.id LIMIT 1) AS channel_id,
               (SELECT count(*) FROM events e WHERE e.guild_id = g.guild_id) AS "event_count!"
        FROM guilds g
        LEFT JOIN guild_config c ON c.guild_id = g.guild_id
        WHERE g.guild_id = $1
        "#,
        guild_id
    )
    .fetch_optional(executor)
    .await
}

#[cfg(test)]
mod tests {
    use super::name_pattern;

    #[test]
    fn escapes_like_metacharacters() {
        assert_eq!(name_pattern("abc"), "%abc%");
        assert_eq!(name_pattern("50%_off\\"), "%50\\%\\_off\\\\%");
    }
}
