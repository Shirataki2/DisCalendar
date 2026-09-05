//! 管理コンソールの定型操作 (#36) のクエリ。
//!
//! 自由 SQL での書き込みは許可しない (#33 の検討) 代わりに、必要な書き込みをここに定型操作として用意する。
//! 呼び出し側 (routes/admin_ops.rs) が操作と同じトランザクションで `admin_audit_logs` に記録する。

use sqlx::{PgConnection, PgExecutor};

use super::events::EventRow;

/// 全予定削除で監査ログに残すスナップショットの上限件数。それ以上は件数だけ記録する
/// (大量の予定を一括で読み込んで JSONB 1 値に詰めると、api のメモリと監査ログの挿入が破綻して削除自体も失敗するため)
pub const DELETE_SNAPSHOT_LIMIT: i64 = 200;

/// ギルドの予定行をすべてロックする。全予定削除の前に呼び、Discord 連携の対応付け (#94) を
/// 控えてから削除するまでの間に、並行トランザクションが既存の予定に連携を足す隙を無くす
/// (連携の追加・変更は予定行の `FOR UPDATE` を取ってから行われるため、ここで待たされる)。
/// 削除と並行して新しく作られる予定は行ロックでは待たせられないので、呼び出し側は
/// この前に [`super::event_links::lock_guild`] でギルド単位の勧告ロックを取り、
/// 連携付きの新規作成と排他する
pub async fn lock_guild_events<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
) -> sqlx::Result<()> {
    sqlx::query!(
        "SELECT id FROM events WHERE guild_id = $1 FOR UPDATE",
        guild_id
    )
    .fetch_all(executor)
    .await?;
    Ok(())
}

/// 指定ギルドの予定をすべて削除する。戻り値は (スナップショット (先頭 `DELETE_SNAPSHOT_LIMIT` 件、id 順), 削除件数)。
/// 呼び出し側のトランザクションの中で動かす。スナップショットの行は `FOR UPDATE` でロックしてから消すので、
/// 監査ログに残る内容と実際に消えた行が (同時に更新されても) 一致する
pub async fn delete_guild_events(
    conn: &mut PgConnection,
    guild_id: &str,
) -> sqlx::Result<(Vec<EventRow>, u64)> {
    let snapshot = sqlx::query_as!(
        EventRow,
        r#"
        SELECT e.id, e.guild_id, e.name, e.description, e.notifications, e.color, e.is_all_day,
               e.start_at, e.end_at, e.created_at, e.created_by, e.updated_by, e.updated_at,
               l.scheduled_event_id AS "discord_scheduled_event_id?"
        FROM events e
        LEFT JOIN event_discord_links l ON l.event_id = e.id
        WHERE e.guild_id = $1
        ORDER BY e.id
        LIMIT $2
        FOR UPDATE OF e
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
