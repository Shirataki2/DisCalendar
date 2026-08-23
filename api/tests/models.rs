//! DB を使う統合テスト。`#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する。
//! 実行には `DATABASE_URL` (api/.env でも可) で接続できる Postgres が必要。

use chrono::NaiveDateTime;
use discalendar_api::models::{
    events::{self, EventInput},
    guilds,
    notifications::{Notification, NotificationUnit},
};
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";

fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}

fn input(name: &str, start: &str, end: &str) -> EventInput {
    EventInput {
        name: name.to_owned(),
        description: Some("desc".to_owned()),
        notifications: vec![Notification {
            num: 30,
            unit: NotificationUnit::Minutes,
        }],
        color: "#2196F3".to_owned(),
        is_all_day: false,
        start_at: dt(start),
        end_at: dt(end),
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn event_crud_is_scoped_to_guild(pool: PgPool) {
    let created = events::create(
        &pool,
        GUILD,
        &input("meeting", "2026-08-22T10:00:00", "2026-08-22T11:00:00"),
        dt("2026-08-01T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(created.guild_id, GUILD);
    // 通知は旧 Bot が読む形式で保存される
    assert_eq!(
        created.notifications,
        vec![r#"{"key":0,"num":30,"type":"分前"}"#]
    );

    // 他ギルドからは更新・削除できない
    let mut renamed = input("renamed", "2026-08-22T10:00:00", "2026-08-22T12:00:00");
    renamed.notifications.clear();
    assert!(
        events::update(&pool, OTHER_GUILD, created.id, &renamed)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !events::delete(&pool, OTHER_GUILD, created.id)
            .await
            .unwrap()
    );

    let updated = events::update(&pool, GUILD, created.id, &renamed)
        .await
        .unwrap()
        .expect("event exists");
    assert_eq!(updated.name, "renamed");
    assert_eq!(updated.end_at, dt("2026-08-22T12:00:00"));
    assert!(updated.notifications.is_empty());
    // created_at は更新で変わらない
    assert_eq!(updated.created_at, created.created_at);

    assert!(events::delete(&pool, GUILD, created.id).await.unwrap());
    assert!(!events::delete(&pool, GUILD, created.id).await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn list_returns_events_overlapping_range(pool: PgPool) {
    let now = dt("2026-08-01T00:00:00");
    // 範囲より前に始まり範囲内で終わる
    events::create(
        &pool,
        GUILD,
        &input("spans-into", "2026-08-30T10:00:00", "2026-09-02T10:00:00"),
        now,
    )
    .await
    .unwrap();
    // 範囲内
    events::create(
        &pool,
        GUILD,
        &input("inside", "2026-09-10T10:00:00", "2026-09-10T11:00:00"),
        now,
    )
    .await
    .unwrap();
    // 範囲の終端ちょうどに始まる (end は含まない)
    events::create(
        &pool,
        GUILD,
        &input("at-end", "2026-10-01T00:00:00", "2026-10-01T01:00:00"),
        now,
    )
    .await
    .unwrap();
    // 範囲より前
    events::create(
        &pool,
        GUILD,
        &input("before", "2026-08-01T10:00:00", "2026-08-01T11:00:00"),
        now,
    )
    .await
    .unwrap();
    // 他ギルド
    events::create(
        &pool,
        OTHER_GUILD,
        &input("other", "2026-09-10T10:00:00", "2026-09-10T11:00:00"),
        now,
    )
    .await
    .unwrap();

    let rows = events::list_between(
        &pool,
        GUILD,
        dt("2026-09-01T00:00:00"),
        dt("2026-10-01T00:00:00"),
    )
    .await
    .unwrap();
    let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["spans-into", "inside"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn guild_config_defaults_and_upserts(pool: PgPool) {
    let config = guilds::get_config(&pool, GUILD).await.unwrap();
    assert!(!config.restricted);
    assert_eq!(config.guild_id, GUILD);

    let config = guilds::upsert_config(&pool, GUILD, true).await.unwrap();
    assert!(config.restricted);
    assert!(guilds::get_config(&pool, GUILD).await.unwrap().restricted);

    let config = guilds::upsert_config(&pool, GUILD, false).await.unwrap();
    assert!(!config.restricted);
}

#[sqlx::test(migrations = "./migrations")]
async fn joined_guilds_filters_by_bot_registry(pool: PgPool) {
    sqlx::query(
        "INSERT INTO guilds (guild_id, name, avatar_url, locale) VALUES ($1, 'A', NULL, 'ja')",
    )
    .bind(GUILD)
    .execute(&pool)
    .await
    .unwrap();

    let joined = guilds::find_joined(&pool, &[GUILD.to_owned(), OTHER_GUILD.to_owned()])
        .await
        .unwrap();
    assert_eq!(joined.len(), 1);
    assert_eq!(joined[0].guild_id, GUILD);
    assert_eq!(joined[0].name, "A");

    assert!(
        guilds::find_by_guild_id(&pool, OTHER_GUILD)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_audit_log_is_recorded(pool: PgPool) {
    use discalendar_api::{
        auth::AuthUser,
        models::admin_audit::{self, AuditEntry},
    };
    use serde_json::json;

    let actor = AuthUser {
        id: "user_1".to_owned(),
        name: "admin".to_owned(),
        discord_user_id: "123456789012345678".to_owned(),
    };
    let entry = AuditEntry {
        action: "event.update",
        target_type: Some("event"),
        target_id: Some("42"),
        before: Some(json!({ "name": "old" })),
        after: Some(json!({ "name": "new" })),
        detail: None,
    };
    let log = admin_audit::record(&pool, &actor, entry).await.unwrap();
    assert_eq!(log.actor_user_id, "user_1");
    assert_eq!(log.actor_discord_user_id, "123456789012345678");
    assert_eq!(log.action, "event.update");
    assert_eq!(log.target_type.as_deref(), Some("event"));
    assert_eq!(log.target_id.as_deref(), Some("42"));
    assert_eq!(log.before, Some(json!({ "name": "old" })));
    assert_eq!(log.after, Some(json!({ "name": "new" })));
    assert!(log.detail.is_none());

    // 対象なし・スナップショットなし (SQL 実行など) でも書ける
    let log = admin_audit::record(
        &pool,
        &actor,
        AuditEntry {
            action: "sql.select",
            detail: Some(json!({ "sql": "SELECT 1", "rows": 1 })),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(log.target_type.is_none());
    assert_eq!(log.detail, Some(json!({ "sql": "SELECT 1", "rows": 1 })));

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_audit_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}
