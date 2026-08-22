use sqlx::{PgExecutor, PgPool};

/// 予定の通知先チャンネル (`event_settings` テーブル)。`/init` で設定し、通知の定期タスク (#4) が読む
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSettings {
    pub guild_id: String,
    pub channel_id: String,
}

/// ギルドの通知先。未設定なら `None`。
/// 旧スキーマには `guild_id` の一意制約がないので、複数行あっても先頭の 1 行を使う
pub async fn get(pool: &PgPool, guild_id: &str) -> sqlx::Result<Option<EventSettings>> {
    fetch(pool, guild_id).await
}

async fn fetch<'e>(
    executor: impl PgExecutor<'e>,
    guild_id: &str,
) -> sqlx::Result<Option<EventSettings>> {
    sqlx::query_as!(
        EventSettings,
        "SELECT guild_id, channel_id FROM event_settings WHERE guild_id = $1 ORDER BY id LIMIT 1",
        guild_id
    )
    .fetch_optional(executor)
    .await
}

/// 通知先を設定する。既に設定があれば置き換え、なければ追加する。
/// 戻り値は変更前の通知先 (初回設定なら `None`)
pub async fn set(
    pool: &PgPool,
    guild_id: &str,
    channel_id: &str,
) -> sqlx::Result<Option<EventSettings>> {
    let mut tx = pool.begin().await?;
    // 一意制約がないので ON CONFLICT は使えない。同じギルドの /init が同時に走ったときに
    // 両方が「未設定」と判断して 2 行 INSERT しないよう、ギルド ID ごとのアドバイザリロックで
    // 読み取りから書き込みまでを直列化する (ロックはトランザクション終了時に自動で外れる)
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(guild_id)
        .execute(&mut *tx)
        .await?;
    let previous = fetch(&mut *tx, guild_id).await?;
    if previous.is_some() {
        // 旧 Bot の時代に同じギルドの行が複数できていれば、まとめて更新する
        sqlx::query!(
            "UPDATE event_settings SET channel_id = $2 WHERE guild_id = $1",
            guild_id,
            channel_id
        )
        .execute(&mut *tx)
        .await?;
    } else {
        sqlx::query!(
            "INSERT INTO event_settings (guild_id, channel_id) VALUES ($1, $2)",
            guild_id,
            channel_id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(previous)
}
