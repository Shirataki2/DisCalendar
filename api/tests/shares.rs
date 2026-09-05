//! 共有リンクの寿命・ギルド境界・公開項目を確認する。
use discalendar_api::models::{
    events::{self, EventInput},
    shares,
};
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn share_lifecycle_and_public_fields(pool: PgPool) {
    sqlx::query("INSERT INTO guilds (guild_id, name, locale) VALUES ('111', '共有サーバー', 'ja')")
        .execute(&pool)
        .await
        .unwrap();
    let input = EventInput {
        name: "共有予定".into(),
        description: Some("説明".into()),
        notifications: vec![],
        color: "#5865F2".into(),
        is_all_day: false,
        start_at: "2026-09-05T10:00:00".parse().unwrap(),
        end_at: "2026-09-05T11:00:00".parse().unwrap(),
        discord_scheduled_event: None,
    };
    let event = events::create(&pool, "111", &input, input.start_at, "333")
        .await
        .unwrap();
    assert!(
        shares::issue(&pool, "222", event.id)
            .await
            .unwrap()
            .is_none()
    );
    let link = shares::issue(&pool, "111", event.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        shares::issue(&pool, "111", event.id)
            .await
            .unwrap()
            .unwrap()
            .token,
        link.token
    );
    let public = shares::find(&pool, &link.token).await.unwrap().unwrap();
    let json = serde_json::to_value(public).unwrap();
    assert_eq!(json["guild_name"], "共有サーバー");
    assert_eq!(json.as_object().unwrap().len(), 8);
    let updated = EventInput {
        name: "変更済み".into(),
        ..input
    };
    events::update(
        &pool,
        "111",
        event.id,
        &updated,
        "333",
        "2026-09-05T12:00:00".parse().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        shares::find(&pool, &link.token)
            .await
            .unwrap()
            .unwrap()
            .name,
        "変更済み"
    );
    shares::revoke(&pool, "222", event.id).await.unwrap();
    assert!(shares::find(&pool, &link.token).await.unwrap().is_some());
    shares::revoke(&pool, "111", event.id).await.unwrap();
    assert!(shares::find(&pool, &link.token).await.unwrap().is_none());
    let next = shares::issue(&pool, "111", event.id)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(link.token, next.token);
    sqlx::query("DELETE FROM guilds WHERE guild_id = '111'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(shares::find(&pool, &next.token).await.unwrap().is_none());
    events::delete(&pool, "111", event.id).await.unwrap();
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM event_share_links")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}
