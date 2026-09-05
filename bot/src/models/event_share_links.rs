use sqlx::PgPool;

/// 明示的に発行された共有リンクだけを取得する。通知のために発行・更新はしない。
pub async fn get_token(
    pool: &PgPool,
    guild_id: &str,
    event_id: i32,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar!(
        "SELECT s.token FROM event_share_links s JOIN events e ON e.id = s.event_id WHERE e.guild_id = $1 AND e.id = $2",
        guild_id,
        event_id
    )
    .fetch_optional(pool)
    .await
}
