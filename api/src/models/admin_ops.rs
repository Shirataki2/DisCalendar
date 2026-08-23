//! 管理コンソールの定型操作 (#36) のクエリ。
//!
//! 自由 SQL での書き込みは許可しない (#33 の検討) 代わりに、必要な書き込みをここに定型操作として用意する。
//! 呼び出し側 (routes/admin_ops.rs) が操作と同じトランザクションで `admin_audit_logs` に記録する。

use sqlx::PgExecutor;

use super::events::EventRow;

/// 指定ギルドの予定をすべて削除し、削除した行を返す (監査ログのスナップショット用)
pub async fn delete_guild_events<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
) -> sqlx::Result<Vec<EventRow>> {
    sqlx::query_as!(
        EventRow,
        r#"
        DELETE FROM events
        WHERE guild_id = $1
        RETURNING id, guild_id, name, description, notifications, color, is_all_day,
                  start_at, end_at, created_at
        "#,
        guild_id
    )
    .fetch_all(executor)
    .await
}

/// Better Auth の `session` のうち期限切れのものを消し、件数を返す。
/// Better Auth は期限切れセッションを参照時に消すだけなので、使われなくなった行が溜まる
pub async fn purge_expired_sessions<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<u64> {
    let result = sqlx::query!(r#"DELETE FROM "session" WHERE "expiresAt" < now()"#)
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}
