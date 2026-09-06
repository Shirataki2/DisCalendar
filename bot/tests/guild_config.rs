//! `guild_config` テーブル (web のサーバー設定が書く restricted モード) の読み取り

use discalendar_bot::models::guild_config;
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";

#[sqlx::test(migrations = "../api/migrations")]
async fn is_restricted_defaults_to_false_and_reads_web_setting(pool: PgPool) {
    assert!(!guild_config::get(&pool, GUILD).await.unwrap().restricted);

    sqlx::query!(
        "INSERT INTO guild_config (guild_id, restricted) VALUES ($1, TRUE)",
        GUILD
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(guild_config::get(&pool, GUILD).await.unwrap().restricted);

    sqlx::query!(
        "UPDATE guild_config SET restricted = FALSE WHERE guild_id = $1",
        GUILD
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(!guild_config::get(&pool, GUILD).await.unwrap().restricted);
}

#[sqlx::test(migrations = "../api/migrations")]
async fn editor_roles_match_api_authorization(pool: PgPool) {
    let default = guild_config::get(&pool, GUILD).await.unwrap();
    assert!(default.editor_role_ids.is_empty());
    assert!(default.can_edit_events(false, &[]));
    sqlx::query("INSERT INTO guild_config (guild_id, restricted, editor_role_ids) VALUES ($1, TRUE, ARRAY['123'])")
        .bind(GUILD).execute(&pool).await.unwrap();
    let mut config = guild_config::get(&pool, GUILD).await.unwrap();
    for (restricted, manager, roles, expected) in [
        (false, false, vec![], true),
        (false, false, vec!["456".into()], true),
        (true, false, vec![], false),
        (true, false, vec!["456".into()], false),
        (true, false, vec!["456".into(), "123".into()], true),
        (true, true, vec![], true),
    ] {
        config.restricted = restricted;
        assert_eq!(config.can_edit_events(manager, &roles), expected);
    }
}
