//! ギルドの予定の有無 (#377) の DB テスト。
//! `#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する (Postgres が必要)。

use discalendar_api::models::events;
use sqlx::PgPool;

async fn exec(pool: &PgPool, sql: &'static str) {
    sqlx::raw_sql(sql).execute(pool).await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn has_events_is_false_without_any_event(pool: PgPool) {
    // 他のギルドの予定は関係ない
    exec(
        &pool,
        "INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at)
         VALUES ('2', 'a', '[]'::jsonb, false, '2026-11-10T10:00:00', '2026-11-10T11:00:00')",
    )
    .await;
    assert!(!events::guild_has_events(&pool, "1").await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn single_events_count_even_in_the_trash(pool: PgPool) {
    exec(
        &pool,
        "INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at, deleted_at)
         VALUES ('1', 'a', '[]'::jsonb, false, '2026-11-10T10:00:00', '2026-11-10T11:00:00',
                 '2026-11-11T00:00:00')",
    )
    .await;
    assert!(events::guild_has_events(&pool, "1").await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn recurring_series_count(pool: PgPool) {
    exec(
        &pool,
        "INSERT INTO event_series (guild_id, template, recurrence, rrule, start_at, end_at,
                                   created_at, created_by)
         VALUES ('1', '{}'::jsonb, '{}'::jsonb, 'FREQ=WEEKLY', '2026-11-10T10:00:00',
                 '2026-11-10T11:00:00', '2026-11-01T09:00:00', 'u1')",
    )
    .await;
    assert!(events::guild_has_events(&pool, "1").await.unwrap());
}
