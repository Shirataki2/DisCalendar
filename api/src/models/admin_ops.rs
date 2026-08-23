//! 管理コンソールの定型操作 (#36) のクエリ。
//!
//! 自由 SQL での書き込みは許可しない (#33 の検討) 代わりに、必要な書き込みをここに定型操作として用意する。
//! 呼び出し側 (routes/admin_ops.rs) が操作と同じトランザクションで `admin_audit_logs` に記録する。

use sqlx::{PgConnection, PgExecutor};

use super::events::EventRow;

/// 全予定削除で監査ログに残すスナップショットの上限件数。それ以上は件数だけ記録する
/// (大量の予定を一括で読み込んで JSONB 1 値に詰めると、api のメモリと監査ログの挿入が破綻して削除自体も失敗するため)
pub const DELETE_SNAPSHOT_LIMIT: i64 = 200;

/// 指定ギルドの予定をすべて削除する。戻り値は (スナップショット (先頭 `DELETE_SNAPSHOT_LIMIT` 件、id 順), 削除件数)。
/// 呼び出し側のトランザクションの中で動かす
pub async fn delete_guild_events(
    conn: &mut PgConnection,
    guild_id: &str,
) -> sqlx::Result<(Vec<EventRow>, u64)> {
    let snapshot = sqlx::query_as!(
        EventRow,
        r#"
        SELECT id, guild_id, name, description, notifications, color, is_all_day,
               start_at, end_at, created_at
        FROM events
        WHERE guild_id = $1
        ORDER BY id
        LIMIT $2
        "#,
        guild_id,
        DELETE_SNAPSHOT_LIMIT
    )
    .fetch_all(&mut *conn)
    .await?;
    let deleted = sqlx::query!("DELETE FROM events WHERE guild_id = $1", guild_id)
        .execute(&mut *conn)
        .await?
        .rows_affected();
    Ok((snapshot, deleted))
}

/// Better Auth の `session` のうち期限切れのものを消し、件数を返す。
/// Better Auth は期限切れセッションを参照時に消すだけなので、使われなくなった行が溜まる
pub async fn purge_expired_sessions<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<u64> {
    let result = sqlx::query!(r#"DELETE FROM "session" WHERE "expiresAt" < now()"#)
        .execute(executor)
        .await?;
    Ok(result.rows_affected())
}
