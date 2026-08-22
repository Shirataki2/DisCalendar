//! `guild_config` テーブル (web のサーバー設定が書く restricted モード) の読み取り

use discalendar_bot::models::guild_config;
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";

#[sqlx::test(migrations = "../api/migrations")]
async fn is_restricted_defaults_to_false_and_reads_web_setting(pool: PgPool) {
    assert!(!guild_config::is_restricted(&pool, GUILD).await.unwrap());

    sqlx::query!(
        "INSERT INTO guild_config (guild_id, restricted) VALUES ($1, TRUE)",
        GUILD
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(guild_config::is_restricted(&pool, GUILD).await.unwrap());

    sqlx::query!(
        "UPDATE guild_config SET restricted = FALSE WHERE guild_id = $1",
        GUILD
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(!guild_config::is_restricted(&pool, GUILD).await.unwrap());
}
