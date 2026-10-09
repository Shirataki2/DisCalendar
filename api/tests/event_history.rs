//! 予定の変更履歴 (#165) の統合テスト。`#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する。
//! ルートの各ハンドラは予定の書き込みと同じトランザクションで `event_history::record` を呼ぶので、
//! ここではその前提 (中身・同一トランザクション・ギルドでの絞り込み・上限) を確かめる。

use chrono::NaiveDateTime;
use discalendar_api::{
    event_history::{self, Action, MAX_PER_EVENT, Source},
    models::{
        event_history as history, event_links,
        events::{self, EventInput},
        notifications::{Notification, NotificationUnit},
        now_jst,
    },
    recurring::{self, ChangeScope},
    recurring_events,
};
use serde_json::json;
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";
const ACTOR: &str = "333333333333333333";

fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}

fn input(name: &str) -> EventInput {
    EventInput {
        recurrence: None,
        scope: Default::default(),
        expected_series_version: None,
        name: name.to_owned(),
        description: None,
        location: None,
        notifications: vec![Notification {
            num: 10,
            unit: NotificationUnit::Minutes,
        }],
        color: "#2196F3".to_owned(),
        is_all_day: false,
        start_at: dt("2026-09-05T21:00:00"),
        end_at: dt("2026-09-05T22:00:00"),
        discord_scheduled_event: None,
        notification_mentions: None,
    }
}

/// ルートの作成と同じ手順 (作成 → 同じトランザクションで履歴)
async fn create(pool: &PgPool, guild: &str, name: &str) -> i32 {
    let mut tx = pool.begin().await.unwrap();
    let row = events::create(&mut *tx, guild, &input(name), now_jst(), ACTOR)
        .await
        .unwrap();
    event_history::record(
        &mut tx,
        guild,
        row.id,
        Some(ACTOR),
        Source::Web,
        Action::Create,
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    row.id
}

/// ルートの更新と同じ手順 (変更前を読む → 更新 → 同じトランザクションで履歴)
async fn update(pool: &PgPool, id: i32, body: &EventInput) {
    let mut tx = pool.begin().await.unwrap();
    let before = event_history::snapshot(&mut tx, GUILD, id).await.unwrap();
    events::update(&mut *tx, GUILD, id, body, ACTOR, now_jst())
        .await
        .unwrap()
        .unwrap();
    event_history::record(
        &mut tx,
        GUILD,
        id,
        Some(ACTOR),
        Source::Web,
        Action::Update,
        before.as_ref(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn create_and_update_record_snapshots(pool: PgPool) {
    let id = create(&pool, GUILD, "定例").await;
    let mut body = input("定例 (延期)");
    body.start_at = dt("2026-09-06T21:00:00");
    body.end_at = dt("2026-09-06T22:00:00");
    update(&pool, id, &body).await;

    let entries = history::list(&pool, GUILD, id).await.unwrap();
    assert_eq!(entries.len(), 2);
    // 新しい順
    let (updated, created) = (&entries[0], &entries[1]);
    assert_eq!(created.action, "create");
    assert_eq!(created.source, "web");
    assert_eq!(created.actor_discord_user_id.as_deref(), Some(ACTOR));
    assert!(created.before.is_none());
    assert_eq!(
        created.after,
        Some(json!({
            "name": "定例",
            "description_hash": null,
            "location": null,
            "color": "#2196F3",
            "is_all_day": false,
            "start_at": "2026-09-05T21:00:00",
            "end_at": "2026-09-05T22:00:00",
            "notifications": [{"num": 10, "unit": "minutes"}],
            "notification_mentions": [],
            "discord_linked": false,
        }))
    );

    assert_eq!(updated.action, "update");
    assert_eq!(updated.before, created.after);
    let after = updated.after.as_ref().unwrap();
    assert_eq!(after["name"], "定例 (延期)");
    assert_eq!(after["start_at"], "2026-09-06T21:00:00");
    assert_eq!(after["end_at"], "2026-09-06T22:00:00");
}

#[sqlx::test(migrations = "./migrations")]
async fn unchanged_updates_are_not_recorded(pool: PgPool) {
    let id = create(&pool, GUILD, "定例").await;
    update(&pool, id, &input("定例")).await;
    let entries = history::list(&pool, GUILD, id).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].action, "create");
}

#[sqlx::test(migrations = "./migrations")]
async fn discord_link_is_part_of_the_snapshot(pool: PgPool) {
    let id = create(&pool, GUILD, "定例").await;
    // 値は同じでも連携の有無が変われば履歴に残る
    let mut tx = pool.begin().await.unwrap();
    let before = event_history::snapshot(&mut tx, GUILD, id).await.unwrap();
    event_links::insert(&mut *tx, GUILD, id, "444444444444444444", now_jst())
        .await
        .unwrap();
    event_history::record(
        &mut tx,
        GUILD,
        id,
        Some(ACTOR),
        Source::Web,
        Action::Update,
        before.as_ref(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let entries = history::list(&pool, GUILD, id).await.unwrap();
    assert_eq!(entries[0].before.as_ref().unwrap()["discord_linked"], false);
    assert_eq!(entries[0].after.as_ref().unwrap()["discord_linked"], true);
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_writes_leave_no_history(pool: PgPool) {
    let id = create(&pool, GUILD, "定例").await;
    // 更新と履歴を書いた後でロールバックする (後続の処理が失敗した場合)
    let mut tx = pool.begin().await.unwrap();
    let before = event_history::snapshot(&mut tx, GUILD, id).await.unwrap();
    events::update(&mut *tx, GUILD, id, &input("変更"), ACTOR, now_jst())
        .await
        .unwrap();
    event_history::record(
        &mut tx,
        GUILD,
        id,
        Some(ACTOR),
        Source::Web,
        Action::Update,
        before.as_ref(),
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(history::list(&pool, GUILD, id).await.unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn history_is_scoped_to_guild(pool: PgPool) {
    let id = create(&pool, GUILD, "定例").await;
    let other = create(&pool, OTHER_GUILD, "他のサーバー").await;
    assert!(
        history::list(&pool, OTHER_GUILD, id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(history::list(&pool, GUILD, other).await.unwrap().is_empty());
    // 他ギルドの予定 ID を指定しても書き込まれない
    let mut tx = pool.begin().await.unwrap();
    event_history::record(
        &mut tx,
        GUILD,
        other,
        Some(ACTOR),
        Source::Web,
        Action::Update,
        None,
    )
    .await
    .unwrap();
    assert!(
        event_history::snapshot(&mut tx, GUILD, other)
            .await
            .unwrap()
            .is_none()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        history::list(&pool, OTHER_GUILD, other)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn old_entries_are_pruned(pool: PgPool) {
    let id = create(&pool, GUILD, "0").await;
    for i in 1..=MAX_PER_EVENT + 5 {
        update(&pool, id, &input(&i.to_string())).await;
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM event_history WHERE event_id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, MAX_PER_EVENT);
    // 残るのは新しい方
    let latest = history::list(&pool, GUILD, id).await.unwrap();
    assert_eq!(
        latest[0].after.as_ref().unwrap()["name"],
        (MAX_PER_EVENT + 5).to_string()
    );

    // 保持期間を過ぎた行は定期処理で消える
    sqlx::query("UPDATE event_history SET created_at = now() - INTERVAL '181 days' WHERE id = $1")
        .bind(latest[0].id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(event_history::prune_expired(&pool).await.unwrap(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn history_is_deleted_with_the_event(pool: PgPool) {
    let id = create(&pool, GUILD, "定例").await;
    assert!(events::delete(&pool, GUILD, id).await.unwrap());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM event_history")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn description_text_is_not_kept(pool: PgPool) {
    let id = create(&pool, GUILD, "定例").await;
    let mut body = input("定例");
    body.description = Some("合言葉は 1234".to_owned());
    update(&pool, id, &body).await;
    body.description = None;
    update(&pool, id, &body).await;

    let entries = history::list(&pool, GUILD, id).await.unwrap();
    assert_eq!(entries.len(), 3);
    // 説明を消した変更も履歴に残るが、消した本文はどこにも入らない
    let expected: String = sqlx::query_scalar("SELECT md5('合言葉は 1234')")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        entries[0].before.as_ref().unwrap()["description_hash"],
        json!(expected)
    );
    assert_eq!(
        entries[0].after.as_ref().unwrap()["description_hash"],
        json!(null)
    );
    let raw: String =
        sqlx::query_scalar("SELECT string_agg(before::text || after::text, '') FROM event_history")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!raw.contains("合言葉"));
}

/// 「この回以降」の変更は、値が変わった後続の回すべてに履歴を残す
#[sqlx::test(migrations = "./migrations")]
async fn future_scope_updates_are_recorded_on_every_changed_occurrence(pool: PgPool) {
    let mut body: EventInput = serde_json::from_value(json!({
        "name": "定例", "color": "#2196F3",
        "start_at": "2026-09-21T19:30:00", "end_at": "2026-09-21T21:00:00",
        "recurrence_rule": {"frequency": "weekly", "weekdays": [0], "end": {"type": "count", "count": 6}}
    }))
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let row = events::create(&mut *tx, GUILD, &body, body.start_at, ACTOR)
        .await
        .unwrap();
    recurring::attach_created(&mut tx, GUILD, row.id, &body, ACTOR)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    recurring_events::ensure_range(
        &pool,
        GUILD,
        dt("2026-09-01T00:00:00"),
        dt("2026-12-01T00:00:00"),
    )
    .await
    .unwrap();
    let ids: Vec<i32> =
        sqlx::query_scalar("SELECT id FROM events WHERE guild_id = $1 ORDER BY start_at")
            .bind(GUILD)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(ids.len(), 6);

    // 4 回目以降の名前を変える (ルートの更新と同じ手順)
    let target = ids[3];
    let mut tx = pool.begin().await.unwrap();
    let info = recurring_events::info(&mut tx, GUILD, target)
        .await
        .unwrap()
        .unwrap();
    body.name = "定例 (新)".to_owned();
    body.start_at = info.original_start_at;
    body.end_at = info.original_start_at + chrono::Duration::minutes(90);
    body.scope = ChangeScope::Future;
    body.expected_series_version = Some(info.version);
    body.recurrence = None;
    let affected = recurring::affected_ids(&mut tx, GUILD, target, ChangeScope::Future)
        .await
        .unwrap();
    assert_eq!(affected.len(), 3);
    let befores = event_history::snapshots(&mut tx, GUILD, &affected)
        .await
        .unwrap();
    recurring::update(&mut tx, GUILD, target, &body, ACTOR)
        .await
        .unwrap()
        .unwrap();
    event_history::record_many(
        &mut tx,
        GUILD,
        &befores
            .into_iter()
            .map(|(id, before)| (id, Some(before)))
            .collect::<Vec<_>>(),
        Some(ACTOR),
        Source::Web,
        Action::Update,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    for (index, id) in ids.iter().enumerate() {
        let entries = history::list(&pool, GUILD, *id).await.unwrap();
        if index < 3 {
            assert!(entries.is_empty(), "{index} 回目は変わっていない");
        } else {
            assert_eq!(entries.len(), 1, "{index} 回目");
            assert_eq!(entries[0].before.as_ref().unwrap()["name"], "定例");
            assert_eq!(entries[0].after.as_ref().unwrap()["name"], "定例 (新)");
        }
    }
}
