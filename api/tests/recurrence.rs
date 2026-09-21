//! 開催回のID・個別変更・中止が、補充やシリーズ分割で失われないことを確認する。
use chrono::NaiveDateTime;
use discalendar_api::{
    models::events::{self, EventInput},
    recurring::{self, ChangeScope, DeleteOptions},
    recurring_events as store,
};
use sqlx::PgPool;
fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}
fn input() -> EventInput {
    serde_json::from_value(serde_json::json!({"name":"定例","color":"#2196F3","start_at":"2026-09-21T19:30:00","end_at":"2026-09-21T21:00:00","notifications":[{"num":30,"unit":"minutes"}],"recurrence_rule":{"frequency":"weekly","weekdays":[0],"end":{"type":"count","count":8}}})).unwrap()
}
async fn create(pool: &PgPool) -> i32 {
    let mut tx = pool.begin().await.unwrap();
    let body = input();
    body.validate().unwrap();
    let row = events::create(&mut *tx, "111", &body, body.start_at, "333")
        .await
        .unwrap();
    recurring::attach_created(&mut tx, "111", row.id, &body, "333")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    store::ensure_range(
        pool,
        "111",
        dt("2026-09-01T00:00:00"),
        dt("2026-12-01T00:00:00"),
    )
    .await
    .unwrap();
    row.id
}
async fn ids(pool: &PgPool) -> Vec<i32> {
    sqlx::query_scalar("SELECT id FROM events WHERE guild_id='111' ORDER BY start_at")
        .fetch_all(pool)
        .await
        .unwrap()
}
#[sqlx::test(migrations = "./migrations")]
async fn exceptions_survive_split_and_replenishment(pool: PgPool) {
    let first = create(&pool).await;
    let all = ids(&pool).await;
    assert_eq!(all.len(), 8);
    assert_eq!(all[0], first);
    let mut equivalent = input().recurrence.unwrap();
    if let discalendar_api::recurrence::Rule::Weekly { weekdays, .. } = &mut equivalent {
        weekdays.push(0);
    }
    let preview = recurring::preview_dates(
        &pool,
        "111",
        &recurring::PreviewInput {
            event_id: Some(all[6]),
            start_at: dt("2026-11-02T19:30:00"),
            recurrence: equivalent,
        },
    )
    .await
    .unwrap();
    assert_eq!(preview.len(), 2);

    let mut tx = pool.begin().await.unwrap();
    let mut special = input();
    special.recurrence = None;
    special.name = "特別回".into();
    special.start_at = dt("2026-10-06T19:30:00");
    special.end_at = dt("2026-10-06T21:00:00");
    recurring::update(&mut tx, "111", all[2], &special, "333")
        .await
        .unwrap();
    recurring::before_delete(&mut tx, "111", all[3], &DeleteOptions::default())
        .await
        .unwrap();
    events::delete(&mut *tx, "111", all[3]).await.unwrap();
    tx.commit().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let info = store::info(&mut tx, "111", all[1]).await.unwrap().unwrap();
    let mut body = input();
    body.name = "新しい定例".into();
    body.recurrence = None;
    body.scope = ChangeScope::Future;
    body.expected_series_version = Some(info.version);
    body.start_at = dt("2026-09-28T19:30:00");
    body.end_at = dt("2026-09-28T21:00:00");
    recurring::update(&mut tx, "111", all[1], &body, "333")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let (a, b) = tokio::join!(
        store::ensure_range(
            &pool,
            "111",
            dt("2026-09-01T00:00:00"),
            dt("2027-01-01T00:00:00")
        ),
        store::ensure_range(
            &pool,
            "111",
            dt("2026-09-01T00:00:00"),
            dt("2027-01-01T00:00:00")
        )
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(ids(&pool).await.len(), 7);
    let special = events::find_by_id(&pool, "111", all[2])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(special.name, "特別回");
    assert_eq!(special.start_at, dt("2026-10-06T19:30:00"));
    assert!(
        events::find_by_id(&pool, "111", all[3])
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        events::find_by_id(&pool, "111", first)
            .await
            .unwrap()
            .unwrap()
            .name,
        "定例"
    );
    assert_eq!(
        events::find_by_id(&pool, "111", all[4])
            .await
            .unwrap()
            .unwrap()
            .name,
        "新しい定例"
    );
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        recurring::update(&mut tx, "111", all[1], &body, "333").await,
        Err(discalendar_api::error::ApiError::Conflict(_))
    ));
}
#[sqlx::test(migrations = "./migrations")]
async fn future_delete_removes_moved_exception_and_prevents_recreation(pool: PgPool) {
    create(&pool).await;
    let all = ids(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let mut body = input();
    body.recurrence = None;
    body.start_at = dt("2026-09-20T19:30:00");
    body.end_at = dt("2026-09-20T21:00:00");
    recurring::update(&mut tx, "111", all[4], &body, "333")
        .await
        .unwrap();
    let info = store::info(&mut tx, "111", all[1]).await.unwrap().unwrap();
    let options = DeleteOptions {
        scope: ChangeScope::Future,
        expected_series_version: Some(info.version),
    };
    recurring::before_delete(&mut tx, "111", all[1], &options)
        .await
        .unwrap();
    events::delete(&mut *tx, "111", all[1]).await.unwrap();
    tx.commit().await.unwrap();
    store::ensure_range(
        &pool,
        "111",
        dt("2026-09-01T00:00:00"),
        dt("2027-01-01T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(ids(&pool).await, vec![all[0]]);
    let mut tx = pool.begin().await.unwrap();
    assert!(
        recurring::update(&mut tx, "222", all[0], &body, "333")
            .await
            .unwrap()
            .is_none()
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn shifting_to_another_occurrence_keeps_selected_id(pool: PgPool) {
    let id = create(&pool).await;
    let all = ids(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let info = store::info(&mut tx, "111", id).await.unwrap().unwrap();
    let mut body = input();
    body.recurrence = None;
    body.scope = ChangeScope::Future;
    body.expected_series_version = Some(info.version);
    // 3回目から1回目へ戻すと、保持する過去の回と新シリーズが重なるため拒否する。
    assert!(matches!(
        recurring::update(&mut tx, "111", all[2], &body, "333").await,
        Err(discalendar_api::error::ApiError::BadRequest(_))
    ));
    assert_eq!(
        store::info(&mut tx, "111", id)
            .await
            .unwrap()
            .unwrap()
            .version,
        info.version
    );
    body.start_at = dt("2026-09-28T19:30:00");
    body.end_at = dt("2026-09-28T21:00:00");
    let result = recurring::update(&mut tx, "111", id, &body, "333")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(result.id, id);
    tx.commit().await.unwrap();
    store::ensure_range(
        &pool,
        "111",
        dt("2026-09-01T00:00:00"),
        dt("2027-01-01T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(ids(&pool).await.len(), 8);
}

#[sqlx::test(migrations = "./migrations")]
async fn changing_pattern_keeps_the_remaining_count(pool: PgPool) {
    create(&pool).await;
    let all = ids(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    let info = store::info(&mut tx, "111", all[2]).await.unwrap().unwrap();
    let mut body = input();
    body.start_at = dt("2026-10-06T19:30:00");
    body.end_at = dt("2026-10-06T21:00:00");
    body.scope = ChangeScope::Future;
    body.expected_series_version = Some(info.version);
    if let Some(discalendar_api::recurrence::Rule::Weekly { weekdays, .. }) = &mut body.recurrence {
        *weekdays = vec![1];
    }
    recurring::update(&mut tx, "111", all[2], &body, "333")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(ids(&pool).await.len(), 8);
    let mut conn = pool.acquire().await.unwrap();
    let changed = store::info(&mut conn, "111", all[2])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(changed.rule["end"]["count"], 6);
}
#[sqlx::test(migrations = "./migrations")]
async fn rollback_preserves_occurrences(pool: PgPool) {
    create(&pool).await;
    let before = ids(&pool).await;
    sqlx::raw_sql(include_str!(
        "../rollback/20260921000003_revert_generated_range.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../rollback/20260921000002_revert_series_creation.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../rollback/20260921000001_revert_generated_occurrences.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../rollback/20260921000000_revert_event_series.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(ids(&pool).await, before);
    let applied: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version=20260921000000)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!applied);
    sqlx::raw_sql(include_str!(
        "../migrations/20260921000000_create_event_series.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20260921000001_mark_generated_occurrences.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20260921000002_preserve_series_creation.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/20260921000003_track_generated_range.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(ids(&pool).await, before);
    let recurring: i64 =
        sqlx::query_scalar("SELECT count(*) FROM events WHERE series_id IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recurring, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn unmatched_exceptions_and_attachments_keep_ids_then_bulk_delete_stops_series(pool: PgPool) {
    let first = create(&pool).await;
    let all = ids(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO event_attachments(id,event_id,guild_id,filename,size,content_type,temporary_key,object_key,ready) VALUES ('attachment',$1,'111','資料.pdf',1,'application/pdf','tmp','object',true)")
        .bind(all[1]).execute(&mut *tx).await.unwrap();
    let mut special = input();
    special.recurrence = None;
    special.name = "個別変更".into();
    recurring::update(&mut tx, "111", all[2], &special, "333")
        .await
        .unwrap();
    let info = store::info(&mut tx, "111", first).await.unwrap().unwrap();
    let mut body = input();
    body.start_at = dt("2026-09-22T19:30:00");
    body.end_at = dt("2026-09-22T21:00:00");
    body.recurrence = None;
    body.scope = ChangeScope::Future;
    body.expected_series_version = Some(info.version);
    recurring::update(&mut tx, "111", first, &body, "444")
        .await
        .unwrap();
    for id in [all[1], all[2]] {
        assert!(store::info(&mut tx, "111", id).await.unwrap().is_none());
        assert!(
            events::find_by_id_for_update(&mut tx, "111", id)
                .await
                .unwrap()
                .is_some()
        );
    }
    let attachments: i64 = sqlx::query_scalar("SELECT count(*) FROM event_attachments")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(attachments, 1);
    let generated_authors: Vec<Option<String>> =
        sqlx::query_scalar("SELECT created_by FROM events WHERE series_id IS NOT NULL")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert!(
        generated_authors
            .iter()
            .all(|actor| actor.as_deref() == Some("333"))
    );
    let (snapshots, _) =
        discalendar_api::models::admin_ops::delete_guild_events(&mut tx, "111", "333")
            .await
            .unwrap();
    assert!(snapshots.iter().any(|(_, info)| {
        info.as_ref()
            .is_some_and(|i| i.rule["frequency"] == "weekly")
    }));
    tx.commit().await.unwrap();
    store::ensure_range(
        &pool,
        "111",
        dt("2026-09-01T00:00:00"),
        dt("2027-01-01T00:00:00"),
    )
    .await
    .unwrap();
    store::replenish(&pool, dt("2026-09-21T00:00:00"))
        .await
        .unwrap();
    assert!(ids(&pool).await.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn distant_range_is_not_generated_and_feed_keeps_standalone(pool: PgPool) {
    let mut tx = pool.begin().await.unwrap();
    let mut body = input();
    body.is_all_day = true;
    body.start_at = dt("2026-01-01T00:00:00");
    body.end_at = dt("2026-01-03T00:00:00");
    body.recurrence = Some(discalendar_api::recurrence::Rule::MonthlyDate {
        day: 1,
        end: discalendar_api::recurrence::Ending::Never,
    });
    let row = events::create(&mut *tx, "111", &body, body.start_at, "333")
        .await
        .unwrap();
    recurring::attach_created(&mut tx, "111", row.id, &body, "333")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    store::ensure_range(
        &pool,
        "111",
        dt("2040-01-03T00:00:00"),
        dt("2040-01-04T00:00:00"),
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM events WHERE start_at='2040-01-01' AND end_at='2040-01-03'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let old_feed = events::list_for_feed(&pool, "111", dt("2026-01-05T00:00:00"))
        .await
        .unwrap();
    assert!(
        !old_feed
            .iter()
            .any(|event| event.start_at == dt("2026-01-01T00:00:00"))
    );
    // 単発は未来の制限なし、繰り返しの自動生成は現在から730日まで。
    body.recurrence = None;
    body.start_at = dt("2040-02-01T00:00:00");
    body.end_at = body.start_at;
    events::create(&pool, "111", &body, body.start_at, "333")
        .await
        .unwrap();
    let feed = events::list_for_feed(&pool, "111", dt("2026-01-01T00:00:00"))
        .await
        .unwrap();
    assert!(!feed.iter().any(|e| e.start_at == dt("2040-01-01T00:00:00")));
    assert!(feed.iter().any(|e| e.start_at == body.start_at));
}

#[sqlx::test(migrations = "./migrations")]
async fn disabling_series_keeps_target_share_and_emits_one_scoped_webhook(pool: PgPool) {
    sqlx::raw_sql("INSERT INTO guilds(guild_id,name) VALUES ('111','共有'); INSERT INTO guild_webhooks(guild_id,url,kind,secret,created_by) VALUES ('111','https://example.com','json','test','333')").execute(&pool).await.unwrap();
    let id = create(&pool).await;
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_webhook_outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued, 0); // 補充は利用者の作成操作として通知しない。
    let mut created_tx = pool.begin().await.unwrap();
    discalendar_api::webhook_outbox::enqueue(&mut created_tx, "111", id, "event.created", "333")
        .await
        .unwrap();
    let scope: String =
        sqlx::query_scalar("SELECT payload->>'change_scope' FROM guild_webhook_outbox")
            .fetch_one(&mut *created_tx)
            .await
            .unwrap();
    assert_eq!(scope, "future");
    created_tx.rollback().await.unwrap();
    let link = discalendar_api::models::shares::issue(&pool, "111", id)
        .await
        .unwrap()
        .unwrap();
    let rows = events::list_for_feed(&pool, "111", dt("2026-01-01T00:00:00"))
        .await
        .unwrap();
    let guild = discalendar_api::models::guilds::find_by_guild_id(&pool, "111")
        .await
        .unwrap()
        .unwrap();
    let ical = discalendar_api::ical::render_feed(
        &guild,
        &rows,
        "https://example.com",
        chrono::Utc::now(),
    );
    assert_eq!(ical.matches("BEGIN:VEVENT").count(), 8);
    assert!(!ical.contains("RRULE:"));
    assert!(ical.contains(&format!("UID:event-{id}@discalendar.app")));
    let mut tx = pool.begin().await.unwrap();
    let before = store::info(&mut tx, "111", id).await.unwrap().unwrap();
    let mut body = input();
    body.recurrence = Some(discalendar_api::recurrence::Rule::None);
    body.scope = ChangeScope::Future;
    body.expected_series_version = Some(before.version);
    recurring::update(&mut tx, "111", id, &body, "333")
        .await
        .unwrap();
    discalendar_api::webhook_outbox::enqueue_with_scope(
        &mut tx,
        "111",
        id,
        "event.updated",
        "333",
        Some("future"),
        Some(&before),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let payloads: Vec<serde_json::Value> =
        sqlx::query_scalar("SELECT payload FROM guild_webhook_outbox")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["change_scope"], "future");
    assert_eq!(payloads[0]["recurrence"]["series_id"], before.series_id);
    assert_eq!(ids(&pool).await, vec![id]);
    assert!(
        discalendar_api::models::shares::find(&pool, &link.token)
            .await
            .unwrap()
            .is_some()
    );
}

#[test]
fn recurring_input_bounds_and_all_day_storage_are_validated() {
    let mut body = input();
    body.is_all_day = true;
    assert!(body.validate().is_err());
    body.start_at = dt("2026-09-21T00:00:00");
    body.end_at = dt("2026-09-22T00:00:00");
    assert!(body.validate().is_ok());
    body.notifications[0].num = 101;
    assert!(body.validate().is_err());
    body.notifications[0].num = 100;
    body.discord_scheduled_event = Some(true);
    assert!(body.validate().is_err());
}

#[sqlx::test(migrations = "./migrations")]
async fn replenishment_does_not_inflate_creation_metrics_or_lock_finished_series(pool: PgPool) {
    create(&pool).await;
    let now = input().start_at + chrono::Duration::seconds(1);
    let (creation, _) = discalendar_api::models::admin_analytics::event_creation(&pool, now)
        .await
        .unwrap();
    assert_eq!(creation.total, 8);
    assert_eq!(creation.last_day.current, 1);
    let top = discalendar_api::models::admin_analytics::top_guilds(
        &pool,
        now - chrono::Duration::days(1),
        now,
    )
    .await
    .unwrap();
    assert_eq!(top[0].event_count, 1);
    // 終了したシリーズのロックを別接続が保持していても、翌年の一覧取得は待たない。
    let mut locked = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM event_series FOR UPDATE")
        .fetch_all(&mut *locked)
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        store::ensure_range(
            &pool,
            "111",
            dt("2027-01-01T00:00:00"),
            dt("2027-02-01T00:00:00"),
        ),
    )
    .await
    .unwrap()
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn creation_survives_first_cancellation_and_split(pool: PgPool) {
    use discalendar_api::models::admin_analytics as stats;
    sqlx::raw_sql(
        r#"CREATE TABLE "user" ("createdAt" timestamptz);
        CREATE TABLE "session" ("createdAt" timestamptz,"updatedAt" timestamptz,"userId" text);"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let first = create(&pool).await;
    let all = ids(&pool).await;
    let now = input().start_at + chrono::Duration::seconds(1);
    let mut tx = pool.begin().await.unwrap();
    recurring::before_delete(&mut tx, "111", first, &DeleteOptions::default())
        .await
        .unwrap();
    events::delete(&mut *tx, "111", first).await.unwrap();
    let info = store::info(&mut tx, "111", all[1]).await.unwrap().unwrap();
    let mut body = input();
    body.start_at += chrono::Duration::days(7);
    body.end_at += chrono::Duration::days(7);
    body.scope = ChangeScope::Future;
    body.expected_series_version = Some(info.version);
    recurring::update(&mut tx, "111", all[1], &body, "444")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let (creation, _) = stats::event_creation(&pool, now).await.unwrap();
    assert_eq!(creation.total, 7);
    assert_eq!(creation.last_day.current, 1);
    assert_eq!(
        stats::daily(&pool, now)
            .await
            .unwrap()
            .last()
            .unwrap()
            .events,
        1
    );
    assert_eq!(
        stats::monthly(&pool, now)
            .await
            .unwrap()
            .last()
            .unwrap()
            .events,
        1
    );
    assert_eq!(
        stats::top_guilds(&pool, now - chrono::Duration::days(1), now)
            .await
            .unwrap()[0]
            .event_count,
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn webhook_scope_matches_series_operation(pool: PgPool) {
    use discalendar_api::webhook_outbox::{enqueue, enqueue_with_scope};
    sqlx::raw_sql("INSERT INTO guilds(guild_id,name) VALUES ('111','共有'); INSERT INTO guild_webhooks(guild_id,url,kind,secret,created_by) VALUES ('111','https://example.com','json','test','333')").execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let mut body = input();
    body.recurrence = None;
    let row = events::create(&mut *tx, "111", &body, body.start_at, "333")
        .await
        .unwrap();
    for scope in ["this", "future"] {
        enqueue_with_scope(
            &mut tx,
            "111",
            row.id,
            "event.deleted",
            "333",
            Some(scope),
            None,
        )
        .await
        .unwrap();
    }
    body.recurrence = input().recurrence;
    recurring::update(&mut tx, "111", row.id, &body, "333")
        .await
        .unwrap();
    enqueue_with_scope(
        &mut tx,
        "111",
        row.id,
        "event.updated",
        "333",
        Some("this"),
        None,
    )
    .await
    .unwrap();
    // MCPの既存編集にはscopeがなく、シリーズ作成として扱わない。
    enqueue(&mut tx, "111", row.id, "event.updated", "333")
        .await
        .unwrap();
    let scopes: Vec<Option<String>> =
        sqlx::query_scalar("SELECT payload->>'change_scope' FROM guild_webhook_outbox ORDER BY id")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(
        scopes,
        vec![None, None, Some("future".into()), Some("this".into())]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn daily_fill_and_split_use_bounded_sql_statements(pool: PgPool) {
    sqlx::raw_sql("CREATE TABLE statement_counts(n int NOT NULL); INSERT INTO statement_counts VALUES(0);
        CREATE FUNCTION count_event_statements() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN UPDATE statement_counts SET n=n+1; RETURN NULL; END $$;
        CREATE TRIGGER count_events AFTER INSERT OR UPDATE OR DELETE ON events FOR EACH STATEMENT EXECUTE FUNCTION count_event_statements();
        CREATE TRIGGER count_exceptions AFTER INSERT OR UPDATE OR DELETE ON event_series_exceptions FOR EACH STATEMENT EXECUTE FUNCTION count_event_statements();")
        .execute(&pool).await.unwrap();
    let mut body = input();
    body.start_at = discalendar_api::models::now_jst()
        .date()
        .and_hms_opt(19, 30, 0)
        .unwrap();
    body.end_at = body.start_at + chrono::Duration::hours(1);
    body.recurrence = Some(discalendar_api::recurrence::Rule::Daily {
        end: discalendar_api::recurrence::Ending::Never,
    });
    let mut tx = pool.begin().await.unwrap();
    let row = events::create(&mut *tx, "111", &body, body.start_at, "333")
        .await
        .unwrap();
    recurring::attach_created(&mut tx, "111", row.id, &body, "333")
        .await
        .unwrap();
    let before: Vec<i32> = sqlx::query_scalar("SELECT id FROM events ORDER BY id")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert!(before.len() >= 730);
    let count: i32 = sqlx::query_scalar("SELECT n FROM statement_counts")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(count <= 3, "補充のSQL数: {count}");
    sqlx::query("UPDATE statement_counts SET n=0")
        .execute(&mut *tx)
        .await
        .unwrap();
    body.name = "一括変更".into();
    body.scope = ChangeScope::Future;
    body.expected_series_version = Some(
        store::info(&mut tx, "111", row.id)
            .await
            .unwrap()
            .unwrap()
            .version,
    );
    recurring::update(&mut tx, "111", row.id, &body, "444")
        .await
        .unwrap();
    let after: Vec<i32> =
        sqlx::query_scalar("SELECT id FROM events WHERE name='一括変更' ORDER BY id")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
    assert_eq!(before, after);
    let count: i32 = sqlx::query_scalar("SELECT n FROM statement_counts")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(count <= 8, "分割のSQL数: {count}");
}

#[sqlx::test(migrations = "./migrations")]
async fn repeated_range_fill_does_not_write_existing_occurrences(pool: PgPool) {
    create(&pool).await;
    let from = dt("2026-09-01T00:00:00");
    let to = dt("2026-12-01T00:00:00");
    store::ensure_range(&pool, "111", from, to).await.unwrap();
    sqlx::raw_sql(
        "CREATE TABLE insert_counts(n int NOT NULL); INSERT INTO insert_counts VALUES(0);
        CREATE FUNCTION count_event_inserts() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN UPDATE insert_counts SET n=n+1; RETURN NULL; END $$;
        CREATE TRIGGER count_events AFTER INSERT ON events FOR EACH STATEMENT EXECUTE FUNCTION count_event_inserts();",
    )
    .execute(&pool)
    .await
    .unwrap();
    store::ensure_range(&pool, "111", from, to).await.unwrap();
    let count: i32 = sqlx::query_scalar("SELECT n FROM insert_counts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
