//! 管理コンソール用のギルド一覧・詳細 (#35)。
//!
//! Bot は退出時に `guilds` の行を消す (bot/src/event.rs) が、予定 (`events`) や設定 (`guild_config` /
//! `event_settings`) の行は残る。運用で見たいのはそういう「退出済みだがデータが残っているギルド」も含むので、
//! 一覧の起点は `guilds` だけでなく 4 テーブルに出てくる guild_id の和集合にする。
//! `guilds` に行が無いものは `name` / `avatar_url` / `locale` が null、`registered` が false になる。
//! Bot の現在の参加状況 (Discord API) は routes 側で足す。

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
    /// `guilds` の名前。Bot が退出して行が消えていれば null
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    #[schema(example = "ja")]
    pub locale: Option<String>,
    /// `guilds` に行があるか (Bot が参加中として登録しているか)。false なら退出済み (予定や設定だけが残っている)
    pub registered: bool,
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
/// `page` は 1 始まり。名前順 (退出済みで名前が無いものは末尾)
///
/// 予定数は `GROUP BY guild_id` で 1 回だけ集計する (`events.guild_id` にインデックスが無いので、
/// ギルドごとの相関サブクエリにすると行数分だけ全表走査になる)
pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    q: &str,
    page: i64,
) -> sqlx::Result<Vec<AdminGuild>> {
    let offset = (page.max(1) - 1) * PAGE_SIZE;
    sqlx::query_as!(
        AdminGuild,
        r#"
        WITH known AS (
            SELECT guild_id FROM guilds
            UNION SELECT guild_id FROM guild_config
            UNION SELECT guild_id FROM event_settings
            UNION SELECT guild_id FROM events
        ),
        counts AS (SELECT guild_id, count(*) AS n FROM events GROUP BY guild_id)
        SELECT k.guild_id AS "guild_id!", g.name AS "name?", g.avatar_url AS "avatar_url?", g.locale AS "locale?",
               (g.guild_id IS NOT NULL) AS "registered!",
               COALESCE(c.restricted, false) AS "restricted!",
               (SELECT s.channel_id FROM event_settings s WHERE s.guild_id = k.guild_id ORDER BY s.id LIMIT 1) AS channel_id,
               COALESCE(n.n, 0) AS "event_count!"
        FROM known k
        LEFT JOIN guilds g ON g.guild_id = k.guild_id
        LEFT JOIN guild_config c ON c.guild_id = k.guild_id
        LEFT JOIN counts n ON n.guild_id = k.guild_id
        WHERE $1::text = '' OR k.guild_id = $1 OR g.name ILIKE $2
        ORDER BY g.name NULLS LAST, k.guild_id
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
        WITH known AS (
            SELECT guild_id FROM guilds
            UNION SELECT guild_id FROM guild_config
            UNION SELECT guild_id FROM event_settings
            UNION SELECT guild_id FROM events
        )
        SELECT count(*) AS "count!"
        FROM known k
        LEFT JOIN guilds g ON g.guild_id = k.guild_id
        WHERE $1::text = '' OR k.guild_id = $1 OR g.name ILIKE $2
        "#,
        q,
        name_pattern(q)
    )
    .fetch_one(executor)
    .await
}

/// 詳細。どのテーブルにも guild_id が無ければ `None`
pub async fn find<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
) -> sqlx::Result<Option<AdminGuild>> {
    sqlx::query_as!(
        AdminGuild,
        r#"
        SELECT $1::text AS "guild_id!", g.name AS "name?", g.avatar_url AS "avatar_url?", g.locale AS "locale?",
               (g.guild_id IS NOT NULL) AS "registered!",
               COALESCE(c.restricted, false) AS "restricted!",
               (SELECT s.channel_id FROM event_settings s WHERE s.guild_id = $1 ORDER BY s.id LIMIT 1) AS channel_id,
               (SELECT count(*) FROM events e WHERE e.guild_id = $1) AS "event_count!"
        FROM (SELECT 1) AS one
        LEFT JOIN guilds g ON g.guild_id = $1
        LEFT JOIN guild_config c ON c.guild_id = $1
        WHERE g.guild_id IS NOT NULL
           OR c.guild_id IS NOT NULL
           OR EXISTS (SELECT 1 FROM event_settings s WHERE s.guild_id = $1)
           OR EXISTS (SELECT 1 FROM events e WHERE e.guild_id = $1)
        "#,
        guild_id
    )
    .fetch_optional(executor)
    .await
}

/// 現在または過去に存在した (どれかのテーブルに行がある) ギルドか。
/// 管理 API の書き込み (予定の作成・設定変更) で、打ち間違えた ID に孤立データを作らないための確認に使う
pub async fn exists<'e>(executor: impl PgExecutor<'e>, guild_id: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"
        SELECT (
            EXISTS (SELECT 1 FROM guilds WHERE guild_id = $1)
            OR EXISTS (SELECT 1 FROM guild_config WHERE guild_id = $1)
            OR EXISTS (SELECT 1 FROM event_settings WHERE guild_id = $1)
            OR EXISTS (SELECT 1 FROM events WHERE guild_id = $1)
        ) AS "exists!"
        "#,
        guild_id
    )
    .fetch_one(executor)
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
