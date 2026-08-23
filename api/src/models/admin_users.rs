//! 管理コンソールのユーザー・セッション (`/admin/users*`、#37)。
//!
//! Better Auth のテーブル (`user` / `session` / `account`) を読む。
//! **セッショントークン (`session.token`) と Discord のアクセストークン (`account.accessToken` /
//! `refreshToken`) は決して SELECT しない** (AGENTS.md の P0「秘密情報」)。
//! 画面に必要なのは「誰の」「いつからの」「いつ切れる」セッションかだけで、値そのものは要らない。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use super::like_pattern;

/// 1 ページあたりの件数
pub const PAGE_SIZE: i64 = 50;
/// ページ番号の上限 (`admin_guilds::MAX_PAGE` と同じ理由)
pub const MAX_PAGE: i64 = 1_000_000;
/// 1 ユーザーについて返すセッションの上限 (古いものは切り捨てる)
pub const SESSION_LIMIT: i64 = 100;

/// 一覧の 1 行
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct UserSummary {
    /// Better Auth の `user.id`
    pub id: String,
    pub name: String,
    pub email: String,
    pub image: Option<String>,
    pub created_at: DateTime<Utc>,
    /// 連携済み Discord アカウントのユーザー ID (Snowflake、文字列)。連携が無ければ null
    #[schema(example = "123456789012345678")]
    pub discord_user_id: Option<String>,
    /// 期限内のセッション数 (0 ならログインしていない)
    pub active_sessions: i64,
    /// 期限切れを含むセッション数
    pub sessions: i64,
    /// 最後にセッションが作られた (ログインした) 日時
    pub last_session_at: Option<DateTime<Utc>>,
}

/// セッション 1 件。**トークンは含めない**
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct SessionSummary {
    /// `session.id` (認証に使う `session.token` とは別の値)
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    /// `expires_at` を過ぎている (もう認証には使えない)
    pub expired: bool,
}

/// 一覧・検索。`q` が空なら全件。user.id / Discord ユーザー ID の完全一致か、
/// 名前・メールアドレスの部分一致 (大文字小文字を区別しない)。新規登録が新しい順
pub async fn list<'e>(
    executor: impl PgExecutor<'e>,
    q: &str,
    page: i64,
) -> sqlx::Result<Vec<UserSummary>> {
    let offset = (page.clamp(1, MAX_PAGE) - 1) * PAGE_SIZE;
    sqlx::query_as!(
        UserSummary,
        r#"
        WITH accounts AS (
            SELECT "userId" AS user_id, min("accountId") AS discord_user_id
            FROM "account" WHERE "providerId" = 'discord' GROUP BY "userId"
        ),
        sessions AS (
            SELECT "userId" AS user_id,
                   count(*) FILTER (WHERE "expiresAt" > now()) AS active,
                   count(*) AS total,
                   max("createdAt") AS last_created
            FROM "session" GROUP BY "userId"
        )
        SELECT u.id AS "id!", u.name AS "name!", u.email AS "email!", u.image AS "image?",
               u."createdAt" AS "created_at!",
               a.discord_user_id AS "discord_user_id?",
               COALESCE(s.active, 0) AS "active_sessions!",
               COALESCE(s.total, 0) AS "sessions!",
               s.last_created AS "last_session_at?"
        FROM "user" u
        LEFT JOIN accounts a ON a.user_id = u.id
        LEFT JOIN sessions s ON s.user_id = u.id
        WHERE $1::text = ''
           OR u.id = $1
           OR a.discord_user_id = $1
           OR u.name ILIKE $2
           OR u.email ILIKE $2
        ORDER BY u."createdAt" DESC, u.id
        LIMIT $3 OFFSET $4
        "#,
        q,
        like_pattern(q),
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
        WITH accounts AS (
            SELECT "userId" AS user_id, min("accountId") AS discord_user_id
            FROM "account" WHERE "providerId" = 'discord' GROUP BY "userId"
        )
        SELECT count(*) AS "count!"
        FROM "user" u
        LEFT JOIN accounts a ON a.user_id = u.id
        WHERE $1::text = ''
           OR u.id = $1
           OR a.discord_user_id = $1
           OR u.name ILIKE $2
           OR u.email ILIKE $2
        "#,
        q,
        like_pattern(q)
    )
    .fetch_one(executor)
    .await
}

/// そのユーザーが存在するか (打ち間違えた ID に空の結果を返さず 404 にするための確認)
pub async fn exists<'e>(executor: impl PgExecutor<'e>, user_id: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM "user" WHERE id = $1) AS "exists!""#,
        user_id
    )
    .fetch_one(executor)
    .await
}

/// 指定ユーザーのセッション一覧 (新しい順、[`SESSION_LIMIT`] 件まで)。トークンは返さない
pub async fn sessions<'e>(
    executor: impl PgExecutor<'e>,
    user_id: &str,
) -> sqlx::Result<Vec<SessionSummary>> {
    sqlx::query_as!(
        SessionSummary,
        r#"
        SELECT id AS "id!", "createdAt" AS "created_at!", "updatedAt" AS "updated_at!",
               "expiresAt" AS "expires_at!", "ipAddress" AS "ip_address?", "userAgent" AS "user_agent?",
               ("expiresAt" <= now()) AS "expired!"
        FROM "session"
        WHERE "userId" = $1
        ORDER BY "createdAt" DESC
        LIMIT $2
        "#,
        user_id,
        SESSION_LIMIT
    )
    .fetch_all(executor)
    .await
}

/// 指定ユーザーのセッションをすべて削除して件数を返す (強制ログアウト)。
/// Better Auth はセッションを DB でしか持たないので、これで次のリクエストから未認証になる
pub async fn delete_sessions<'e>(
    executor: impl PgExecutor<'e>,
    user_id: &str,
) -> sqlx::Result<u64> {
    let result = sqlx::query!(r#"DELETE FROM "session" WHERE "userId" = $1"#, user_id)
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}
