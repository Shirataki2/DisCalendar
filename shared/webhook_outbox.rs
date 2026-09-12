//! api / bot 共通。呼び出し元の予定変更と同じトランザクションで予約する。
use sqlx::PgConnection;

/// 呼び出し元で対象の予定行をロックしておく。
/// 削除の場合はこの関数を DELETE より前に呼び、削除前の内容を残す。
pub async fn enqueue(
    conn: &mut PgConnection,
    guild_id: &str,
    event_id: i32,
    kind: &str,
    actor_id: &str,
) -> sqlx::Result<()> {
    // 同時削除された Webhook はロック取得後に除外する。FK 検査だけだと予定保存が失敗する。
    sqlx::query(
        "INSERT INTO guild_webhook_outbox (webhook_id, event_id, kind, payload, actor_id, generation)
         SELECT w.id, e.id, $3, to_jsonb(e) || jsonb_build_object(
             'discord_scheduled_event_id', l.scheduled_event_id), $4, w.generation
         FROM events e
         JOIN guild_webhooks w ON w.guild_id = e.guild_id AND w.enabled
         LEFT JOIN event_discord_links l ON l.event_id = e.id
         WHERE e.guild_id = $1 AND e.id = $2
         FOR KEY SHARE OF w",
    )
    .bind(guild_id)
    .bind(event_id)
    .bind(kind)
    .bind(actor_id)
    .execute(conn)
    .await?;
    Ok(())
}
