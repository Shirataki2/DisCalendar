//! MCP から使う読み取り専用のクエリ (#425)。
//!
//! - サポート用 DB (`discalendar_support`) は `query!` で書く (`support-bot/.sqlx/` で検査する)
//! - 本体 DB は support-bot の `.sqlx/` と別のスキーマなので、実行時のクエリ (`query_as`) で書く
//!
//! 削除済み (`deleted_at`) と保存期間切れ (`purged_at`) の本文は、行に値が残っていても SQL の段階で落とす
//! (Bot 側で NULL にしているが、取りこぼしがあっても MCP からは見せない)。

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::PgPool;

/// チケット一覧で絞る状態
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnresolvedFilter {
    /// open と waiting_user の両方
    All,
    Open,
    WaitingUser,
}

impl UnresolvedFilter {
    fn statuses(self) -> Vec<String> {
        match self {
            Self::All => vec!["open".into(), "waiting_user".into()],
            Self::Open => vec!["open".into()],
            Self::WaitingUser => vec!["waiting_user".into()],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThreadRow {
    pub thread_id: i64,
    pub parent_channel_id: i64,
    pub author_id: Option<i64>,
    /// 削除済み・保存期間切れなら None
    pub title: Option<String>,
    pub status: String,
    pub tags: Vec<String>,
    pub archived: bool,
    pub last_user_msg_at: Option<DateTime<Utc>>,
    pub last_staff_msg_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub github_issue: Option<i32>,
    pub purged_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub message_count: i64,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub message_id: i64,
    pub channel_id: i64,
    pub thread_id: Option<i64>,
    pub author_id: i64,
    pub author_is_staff: bool,
    /// 削除済み・保存期間切れなら None
    pub content: Option<String>,
    pub attachments: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub purged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ReminderRow {
    pub remind_at: DateTime<Utc>,
    pub created_by: i64,
    pub note: Option<String>,
}

/// 未解決 (open / waiting_user) のチケット。利用者の最終発言が古い順 (待たせている順)
pub async fn unresolved_threads(
    pool: &PgPool,
    filter: UnresolvedFilter,
    limit: i64,
) -> Result<Vec<ThreadRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT t.thread_id, t.parent_channel_id, t.author_id,
            CASE WHEN t.deleted_at IS NULL THEN t.title END AS title,
            t.status, t.tags, t.archived, t.last_user_msg_at, t.last_staff_msg_at, t.resolved_at,
            t.github_issue, t.purged_at, t.deleted_at, t.created_at,
            (SELECT count(*) FROM support_messages m WHERE m.thread_id = t.thread_id) AS "message_count!"
        FROM support_threads t
        WHERE t.status = ANY($1) AND t.deleted_at IS NULL
        ORDER BY t.last_user_msg_at NULLS LAST, t.thread_id
        LIMIT $2
        "#,
        &filter.statuses(),
        limit,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| ThreadRow {
            thread_id: r.thread_id,
            parent_channel_id: r.parent_channel_id,
            author_id: r.author_id,
            title: r.title,
            status: r.status,
            tags: r.tags,
            archived: r.archived,
            last_user_msg_at: r.last_user_msg_at,
            last_staff_msg_at: r.last_staff_msg_at,
            resolved_at: r.resolved_at,
            github_issue: r.github_issue,
            purged_at: r.purged_at,
            deleted_at: r.deleted_at,
            created_at: r.created_at,
            message_count: r.message_count,
        })
        .collect())
}

/// チケット 1 件 (削除済みのスレッドも、メタ情報だけは返す)
pub async fn thread(pool: &PgPool, thread_id: i64) -> Result<Option<ThreadRow>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT t.thread_id, t.parent_channel_id, t.author_id,
            CASE WHEN t.deleted_at IS NULL THEN t.title END AS title,
            t.status, t.tags, t.archived, t.last_user_msg_at, t.last_staff_msg_at, t.resolved_at,
            t.github_issue, t.purged_at, t.deleted_at, t.created_at,
            (SELECT count(*) FROM support_messages m WHERE m.thread_id = t.thread_id) AS "message_count!"
        FROM support_threads t
        WHERE t.thread_id = $1
        "#,
        thread_id,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| ThreadRow {
        thread_id: r.thread_id,
        parent_channel_id: r.parent_channel_id,
        author_id: r.author_id,
        title: r.title,
        status: r.status,
        tags: r.tags,
        archived: r.archived,
        last_user_msg_at: r.last_user_msg_at,
        last_staff_msg_at: r.last_staff_msg_at,
        resolved_at: r.resolved_at,
        github_issue: r.github_issue,
        purged_at: r.purged_at,
        deleted_at: r.deleted_at,
        created_at: r.created_at,
        message_count: r.message_count,
    }))
}

/// チケットの発言を古い順に `offset` から `limit` 件
pub async fn thread_messages(
    pool: &PgPool,
    thread_id: i64,
    offset: i64,
    limit: i64,
) -> Result<Vec<MessageRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT m.message_id, m.channel_id, m.thread_id, m.author_id, m.author_is_staff,
            CASE WHEN m.deleted_at IS NULL AND m.purged_at IS NULL AND t.deleted_at IS NULL
                THEN m.content END AS content,
            CASE WHEN m.deleted_at IS NULL AND m.purged_at IS NULL AND t.deleted_at IS NULL
                THEN m.attachments END AS attachments,
            m.created_at, m.edited_at,
            -- スレッドごと削除されたら、発言も削除済みとして扱う
            COALESCE(m.deleted_at, t.deleted_at) AS deleted_at,
            m.purged_at
        FROM support_messages m
        JOIN support_threads t ON t.thread_id = m.thread_id
        WHERE m.thread_id = $1
        ORDER BY m.created_at, m.message_id
        OFFSET $2 LIMIT $3
        "#,
        thread_id,
        offset,
        limit,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| MessageRow {
            message_id: r.message_id,
            channel_id: r.channel_id,
            thread_id: r.thread_id,
            author_id: r.author_id,
            author_is_staff: r.author_is_staff,
            content: r.content,
            attachments: r.attachments,
            created_at: r.created_at,
            edited_at: r.edited_at,
            deleted_at: r.deleted_at,
            purged_at: r.purged_at,
        })
        .collect())
}

/// まだ送っていない `/remind` (メモはスタッフが書いたもの)
pub async fn pending_reminders(
    pool: &PgPool,
    thread_id: i64,
) -> Result<Vec<ReminderRow>, sqlx::Error> {
    sqlx::query_as!(
        ReminderRow,
        r#"
        SELECT remind_at, created_by, note
        FROM support_reminders
        WHERE thread_id = $1 AND sent_at IS NULL
        ORDER BY remind_at
        "#,
        thread_id,
    )
    .fetch_all(pool)
    .await
}

/// 本文の部分一致検索 (大文字小文字を区別しない)。削除済み・保存期間切れの本文は対象にしない。
/// 新しい順。`resolved` が false なら解決済みのチケットとテキストチャンネルの発言を除く
pub async fn search_messages(
    pool: &PgPool,
    query: &str,
    include_resolved: bool,
    limit: i64,
) -> Result<Vec<(MessageRow, Option<String>)>, sqlx::Error> {
    let pattern = format!("%{}%", escape_like(query));
    let rows = sqlx::query!(
        r#"
        SELECT m.message_id, m.channel_id, m.thread_id, m.author_id, m.author_is_staff,
            m.content, m.attachments, m.created_at, m.edited_at, m.deleted_at, m.purged_at,
            t.status AS "thread_status?"
        FROM support_messages m
        LEFT JOIN support_threads t ON t.thread_id = m.thread_id
        WHERE m.deleted_at IS NULL AND m.purged_at IS NULL AND m.content IS NOT NULL
            AND m.content ILIKE $1 ESCAPE '\'
            AND (m.thread_id IS NULL OR t.deleted_at IS NULL)
            AND ($2 OR (m.thread_id IS NOT NULL AND t.status <> 'resolved'))
        ORDER BY m.created_at DESC, m.message_id DESC
        LIMIT $3
        "#,
        pattern,
        include_resolved,
        limit,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                MessageRow {
                    message_id: r.message_id,
                    channel_id: r.channel_id,
                    thread_id: r.thread_id,
                    author_id: r.author_id,
                    author_is_staff: r.author_is_staff,
                    content: r.content,
                    attachments: r.attachments,
                    created_at: r.created_at,
                    edited_at: r.edited_at,
                    deleted_at: r.deleted_at,
                    purged_at: r.purged_at,
                },
                r.thread_status,
            )
        })
        .collect())
}

/// LIKE の特殊文字 (`%` `_` と ESCAPE に使う `\`) を打ち消す
fn escape_like(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len());
    for c in query.chars() {
        if matches!(c, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// 本体 DB から見たギルドの状態
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GuildRow {
    /// guilds に行がある = Bot が参加している (退出すると bot が行を消す)
    pub bot_joined: bool,
    pub name: Option<String>,
    pub locale: Option<String>,
    /// タイムゾーンなしの JST
    pub joined_at: Option<NaiveDateTime>,
    /// guild_config が無ければ None (既定は制限なし)
    pub restricted: Option<bool>,
    /// 予定の通知先 (event_settings)
    pub notification_channel_ids: Vec<String>,
}

/// 本体 DB のギルド。guilds と guild_config のどちらにも無ければ None
pub async fn guild(main: &PgPool, guild_id: &str) -> Result<Option<GuildRow>, sqlx::Error> {
    sqlx::query_as::<_, GuildRow>(
        r#"
        SELECT g.guild_id IS NOT NULL AS bot_joined,
            g.name, g.locale, g.joined_at,
            c.restricted,
            COALESCE(
                (SELECT array_agg(s.channel_id ORDER BY s.id) FROM event_settings s WHERE s.guild_id = $1),
                '{}'
            ) AS notification_channel_ids
        FROM (SELECT $1::text AS guild_id) q
        LEFT JOIN guilds g ON g.guild_id = q.guild_id
        LEFT JOIN guild_config c ON c.guild_id = q.guild_id
        WHERE g.guild_id IS NOT NULL OR c.guild_id IS NOT NULL
        "#,
    )
    .bind(guild_id)
    .fetch_optional(main)
    .await
}

#[cfg(test)]
mod tests {
    use super::escape_like;

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(escape_like(r"100%_a\b"), r"100\%\_a\\b");
        assert_eq!(escape_like("通知"), "通知");
    }
}
