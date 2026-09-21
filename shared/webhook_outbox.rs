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
    enqueue_with_scope(conn, guild_id, event_id, kind, actor_id, None, None).await
}

/// シリーズ操作の範囲を付けて1件だけ通知する。
pub async fn enqueue_with_scope(
    conn: &mut PgConnection,
    guild_id: &str,
    event_id: i32,
    kind: &str,
    actor_id: &str,
    scope: Option<&str>,
    previous_series: Option<&crate::recurring_events::Info>,
) -> sqlx::Result<()> {
    // 同時削除された Webhook はロック取得後に除外する。FK 検査だけだと予定保存が失敗する。
    sqlx::query(
        "INSERT INTO guild_webhook_outbox (webhook_id, event_id, kind, payload, actor_id, generation)
         SELECT w.id, e.id, $3, to_jsonb(e) || jsonb_build_object(
             'discord_scheduled_event_id', l.scheduled_event_id) || CASE WHEN s.id IS NULL AND $6::jsonb IS NULL THEN '{}'::jsonb ELSE jsonb_build_object('recurrence',CASE WHEN s.id IS NULL THEN $6::jsonb ELSE jsonb_build_object('series_id',s.id,'original_start_at',e.original_start_at,'rule',s.recurrence,'version',s.version) END,'change_scope',COALESCE($5::text,'this')) END || CASE WHEN $5::text IS NULL THEN '{}'::jsonb ELSE jsonb_build_object('change_scope',$5::text) END, $4, w.generation
         FROM events e
         JOIN guild_webhooks w ON w.guild_id = e.guild_id AND w.enabled
         LEFT JOIN event_discord_links l ON l.event_id = e.id
         LEFT JOIN event_series s ON s.id = e.series_id
         WHERE e.guild_id = $1 AND e.id = $2
         FOR KEY SHARE OF w",
    )
    .bind(guild_id)
    .bind(event_id)
    .bind(kind)
    .bind(actor_id)
    .bind(scope)
    .bind(previous_series.map(sqlx::types::Json))
    .execute(conn)
    .await?;
    Ok(())
}
