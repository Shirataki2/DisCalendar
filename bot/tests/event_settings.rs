//! `event_settings` テーブル (`/init` の通知先) の読み書き

use discalendar_bot::models::event_settings::{self, EventSettings};
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";

#[sqlx::test(migrations = "../api/migrations")]
async fn set_inserts_then_replaces_and_returns_previous(pool: PgPool) {
    assert_eq!(event_settings::get(&pool, GUILD).await.unwrap(), None);

    // 初回は追加。変更前はない
    let previous = event_settings::set(&pool, GUILD, "1001").await.unwrap();
    assert_eq!(previous, None);
    assert_eq!(
        event_settings::get(&pool, GUILD).await.unwrap(),
        Some(EventSettings {
            guild_id: GUILD.to_owned(),
            channel_id: "1001".to_owned(),
        })
    );

    // 2 回目は置き換え。変更前のチャンネルが返る
    let previous = event_settings::set(&pool, GUILD, "1002").await.unwrap();
    assert_eq!(previous.map(|p| p.channel_id), Some("1001".to_owned()));
    assert_eq!(
        event_settings::get(&pool, GUILD)
            .await
            .unwrap()
            .map(|s| s.channel_id),
        Some("1002".to_owned())
    );

    // 他のギルドには影響しない
    assert_eq!(event_settings::get(&pool, OTHER_GUILD).await.unwrap(), None);
    let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) AS \"count!\" FROM event_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "../api/migrations")]
async fn set_updates_all_rows_when_old_data_has_duplicates(pool: PgPool) {
    // 旧スキーマには一意制約がないので、旧 Bot の時代に同じギルドの行が複数できている可能性がある
    for channel in ["1001", "1002"] {
        sqlx::query!(
            "INSERT INTO event_settings (guild_id, channel_id) VALUES ($1, $2)",
            GUILD,
            channel
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    let previous = event_settings::set(&pool, GUILD, "1003").await.unwrap();
    assert_eq!(previous.map(|p| p.channel_id), Some("1001".to_owned()));
    let channels: Vec<String> = sqlx::query_scalar!(
        "SELECT channel_id FROM event_settings WHERE guild_id = $1",
        GUILD
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(channels, vec!["1003".to_owned(), "1003".to_owned()]);
}
