//! `events` テーブルの読み書き (`/create` と `/list` が使う)。
//! web (api) が保存した予定を Bot が読め、Bot が保存した予定を web が読める形式であることを確認する

use chrono::NaiveDateTime;
use discalendar_bot::models::{
    events::{self, NewEvent},
    notifications::{Notification, NotificationUnit},
};
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";

fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}

fn new_event<'a>(guild_id: &'a str, name: &'a str, start: &str, end: &str) -> NewEvent<'a> {
    NewEvent {
        guild_id,
        name,
        description: None,
        notifications: &[],
        color: "#2196F3",
        is_all_day: false,
        start_at: dt(start),
        end_at: dt(end),
        created_at: dt("2026-08-01T00:00:00"),
        created_by: "333",
    }
}

#[sqlx::test(migrations = "../api/migrations")]
async fn create_stores_notifications_in_legacy_format(pool: PgPool) {
    let notifications = [
        Notification::new(30, NotificationUnit::Minutes),
        Notification::new(1, NotificationUnit::Days),
    ];
    let event = events::create(
        &pool,
        &NewEvent {
            description: Some("説明"),
            notifications: &notifications,
            ..new_event(GUILD, "定例", "2026-08-23T10:00:00", "2026-08-23T11:00:00")
        },
    )
    .await
    .unwrap();

    assert_eq!(event.created_by.as_deref(), Some("333"));
    assert_eq!(event.updated_by, None);
    assert_eq!(event.updated_at, None);
    assert_eq!(event.guild_id, GUILD);
    assert_eq!(event.name, "定例");
    assert_eq!(event.description.as_deref(), Some("説明"));
    // DB には api / web と同じ JSONB で入る
    assert_eq!(
        event.notifications,
        serde_json::json!([
            { "num": 30, "unit": "minutes" },
            { "num": 1, "unit": "days" },
        ])
    );
    assert_eq!(event.notifications(), notifications);
    assert_eq!(event.color, "#2196F3");
    assert!(!event.is_all_day);
    assert_eq!(event.start_at, dt("2026-08-23T10:00:00"));
    assert_eq!(event.end_at, dt("2026-08-23T11:00:00"));
    assert_eq!(event.created_at, dt("2026-08-01T00:00:00"));
}

#[sqlx::test(migrations = "../api/migrations")]
async fn lists_are_scoped_to_guild_and_split_by_now(pool: PgPool) {
    let past = events::create(
        &pool,
        &new_event(GUILD, "過去", "2026-08-01T10:00:00", "2026-08-01T11:00:00"),
    )
    .await
    .unwrap();
    let future = events::create(
        &pool,
        &new_event(GUILD, "未来", "2026-08-30T10:00:00", "2026-08-30T11:00:00"),
    )
    .await
    .unwrap();
    // ちょうど now に始まる予定は過去にも未来にも出る (旧 Bot と同じ <= / >=)
    let now = events::create(
        &pool,
        &new_event(GUILD, "今", "2026-08-15T12:00:00", "2026-08-15T13:00:00"),
    )
    .await
    .unwrap();
    let other_now = events::create(
        &pool,
        &new_event(
            OTHER_GUILD,
            "他",
            "2026-08-15T12:00:00",
            "2026-08-15T13:00:00",
        ),
    )
    .await
    .unwrap();

    let other_future = events::create(
        &pool,
        &new_event(
            OTHER_GUILD,
            "他の未来",
            "2026-08-30T10:00:00",
            "2026-08-30T11:00:00",
        ),
    )
    .await
    .unwrap();

    let at = dt("2026-08-15T12:00:00");
    assert_eq!(
        events::list_all(&pool, GUILD).await.unwrap(),
        vec![past.clone(), now.clone(), future.clone()]
    );
    assert_eq!(
        events::list_past(&pool, GUILD, at).await.unwrap(),
        vec![past, now.clone()]
    );
    assert_eq!(
        events::list_future(&pool, GUILD, at).await.unwrap(),
        vec![now.clone(), future.clone()]
    );
    assert_eq!(
        events::list_all(&pool, "333333333333333333").await.unwrap(),
        vec![]
    );
    // 通知タスクはギルドを横断して未来の予定を集める (start_at, id 順)
    assert_eq!(
        events::list_all_future(&pool, at).await.unwrap(),
        vec![now, other_now, future, other_future]
    );
}

#[sqlx::test(migrations = "../api/migrations")]
async fn reads_events_saved_by_web(pool: PgPool) {
    // web (api) が保存する形: 終日予定は開始日 0:00 〜 終了日 0:00
    sqlx::query!(
        r#"
        INSERT INTO events (guild_id, name, description, notifications, color, is_all_day, start_at, end_at)
        VALUES ($1, 'web の予定', NULL, $2, '#F44336', TRUE, '2026-08-23 00:00:00', '2026-08-24 00:00:00')
        "#,
        GUILD,
        serde_json::json!([{ "num": 1, "unit": "weeks" }, "broken"])
    )
    .execute(&pool)
    .await
    .unwrap();

    let events = events::list_all(&pool, GUILD).await.unwrap();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert!(event.is_all_day);
    assert_eq!(event.start_at, dt("2026-08-23T00:00:00"));
    assert_eq!(event.end_at, dt("2026-08-24T00:00:00"));
    // 解釈できない要素は無視して読む
    assert_eq!(
        event.notifications(),
        vec![Notification::new(1, NotificationUnit::Weeks)]
    );
}
