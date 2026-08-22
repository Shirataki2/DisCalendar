//! DB を使う統合テスト。`#[sqlx::test]` がテストごとに一時 DB を作って api のマイグレーションを適用する。
//! 実行には `DATABASE_URL` (bot/.env でも可) で接続できる Postgres が必要。

use discalendar_bot::models::guilds::{self, Guild};
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";

#[sqlx::test(migrations = "../api/migrations")]
async fn upsert_inserts_then_updates_without_touching_locale(pool: PgPool) {
    guilds::upsert(&pool, GUILD, "DisCalendar", None)
        .await
        .unwrap();
    assert_eq!(
        guilds::find_by_guild_id(&pool, GUILD).await.unwrap(),
        Some(Guild {
            guild_id: GUILD.to_owned(),
            name: "DisCalendar".to_owned(),
            avatar_url: None,
            locale: "ja".to_owned(),
        })
    );

    // 旧 Bot や運用で locale が変えられていても、名前・アイコンの更新で巻き戻さない
    sqlx::query!("UPDATE guilds SET locale = 'en' WHERE guild_id = $1", GUILD)
        .execute(&pool)
        .await
        .unwrap();
    guilds::upsert(
        &pool,
        GUILD,
        "DisCalendar (renamed)",
        Some("https://cdn.discordapp.com/icons/1/a.png"),
    )
    .await
    .unwrap();
    assert_eq!(
        guilds::find_by_guild_id(&pool, GUILD).await.unwrap(),
        Some(Guild {
            guild_id: GUILD.to_owned(),
            name: "DisCalendar (renamed)".to_owned(),
            avatar_url: Some("https://cdn.discordapp.com/icons/1/a.png".to_owned()),
            locale: "en".to_owned(),
        })
    );
}

#[sqlx::test(migrations = "../api/migrations")]
async fn delete_removes_only_that_guild(pool: PgPool) {
    guilds::upsert(&pool, GUILD, "a", None).await.unwrap();
    guilds::upsert(&pool, OTHER_GUILD, "b", None).await.unwrap();

    assert!(guilds::delete(&pool, GUILD).await.unwrap());
    assert!(
        guilds::find_by_guild_id(&pool, GUILD)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        guilds::find_by_guild_id(&pool, OTHER_GUILD)
            .await
            .unwrap()
            .is_some()
    );

    // 2 回目は消す行がない
    assert!(!guilds::delete(&pool, GUILD).await.unwrap());
}

#[sqlx::test(migrations = "../api/migrations")]
async fn delete_many_removes_only_listed_guilds(pool: PgPool) {
    guilds::upsert(&pool, GUILD, "a", None).await.unwrap();
    guilds::upsert(&pool, OTHER_GUILD, "b", None).await.unwrap();
    let mut ids = guilds::list_ids(&pool).await.unwrap();
    ids.sort();
    assert_eq!(ids, vec![GUILD.to_owned(), OTHER_GUILD.to_owned()]);

    // 停止中に退出したギルド (GUILD) と、もともと登録のない ID を渡しても、該当行だけ消える
    let deleted = guilds::delete_many(&pool, &[GUILD.to_owned(), "333333333333333333".to_owned()])
        .await
        .unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(
        guilds::list_ids(&pool).await.unwrap(),
        vec![OTHER_GUILD.to_owned()]
    );

    // 空の一覧では何も消えない
    assert_eq!(guilds::delete_many(&pool, &[]).await.unwrap(), 0);
}
