//! 導入月ごとの定着状況 (#375) の DB テスト。
//! `#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する (Postgres が必要)。
//!
//! 集計の基準時刻は JST の 2026-11-01 12:00 に固定する (直近 30 日 = 10-02 12:00 以降)。

use chrono::{NaiveDate, NaiveDateTime};
use discalendar_api::models::guild_retention::{self, RetentionCohort};
use sqlx::PgPool;

fn now() -> NaiveDateTime {
    "2026-11-01T12:00:00".parse().unwrap()
}

fn month(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

/// | ギルド | 導入 (joined_at) | 予定の作成 | 見たいこと |
/// |---|---|---|---|
/// | g1 | 10-25 00:00 | 10-26 | 7 日経過・7 日以内に作成・直近も作成 |
/// | g2 | 10-30 | 10-31 | 7 日経っていないので割合の分母に入らない |
/// | g3 | 09-01 | 09-05 | 7 日以内・30 日以内。直近は作っていない |
/// | g4 | 09-10 | 09-25, 10-20 | 30 日以内だけ (15 日目)。直近も作成 |
/// | g5 | 09-20 | 09-19 | 導入 (再参加) より前の予定は数えない |
/// | g6 | 09-15 | 09-16 (補充した回) | 繰り返しの補充分は作成に数えない |
/// | g7 | 09-05 | 12-01 | 基準時刻より後の予定は数えない |
/// | g8 | NULL | 10-10 | 導入日不明として件数だけ数える |
/// | g9 | 11-02 | - | 基準時刻より後の導入は数えない |
async fn seed(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO guilds (guild_id, name, joined_at) VALUES
            ('1', 'g1', '2026-10-25T00:00:00'),
            ('2', 'g2', '2026-10-30T09:00:00'),
            ('3', 'g3', '2026-09-01T09:00:00'),
            ('4', 'g4', '2026-09-10T09:00:00'),
            ('5', 'g5', '2026-09-20T09:00:00'),
            ('6', 'g6', '2026-09-15T09:00:00'),
            ('7', 'g7', '2026-09-05T09:00:00'),
            ('8', 'g8', NULL),
            ('9', 'g9', '2026-11-02T09:00:00');
        INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at, created_at,
                            generated_from_series) VALUES
            ('1', 'a', '[]'::jsonb, false, '2026-11-10T10:00:00', '2026-11-10T11:00:00', '2026-10-26T09:00:00', false),
            ('2', 'b', '[]'::jsonb, false, '2026-11-10T10:00:00', '2026-11-10T11:00:00', '2026-10-31T09:00:00', false),
            ('3', 'c', '[]'::jsonb, false, '2026-09-10T10:00:00', '2026-09-10T11:00:00', '2026-09-05T09:00:00', false),
            ('4', 'd', '[]'::jsonb, false, '2026-10-10T10:00:00', '2026-10-10T11:00:00', '2026-09-25T09:00:00', false),
            ('4', 'e', '[]'::jsonb, false, '2026-11-10T10:00:00', '2026-11-10T11:00:00', '2026-10-20T09:00:00', false),
            ('5', 'f', '[]'::jsonb, false, '2026-09-25T10:00:00', '2026-09-25T11:00:00', '2026-09-19T09:00:00', false),
            ('6', 'g', '[]'::jsonb, false, '2026-09-20T10:00:00', '2026-09-20T11:00:00', '2026-09-16T09:00:00', true),
            ('7', 'h', '[]'::jsonb, false, '2026-12-10T10:00:00', '2026-12-10T11:00:00', '2026-12-01T09:00:00', false),
            ('8', 'i', '[]'::jsonb, false, '2026-10-20T10:00:00', '2026-10-20T11:00:00', '2026-10-10T09:00:00', false);
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn cohorts_count_first_events_after_joining(pool: PgPool) {
    seed(&pool).await;

    let retention = guild_retention::guild_retention(&pool, now(), 30)
        .await
        .unwrap();

    assert_eq!(
        retention.cohorts,
        vec![
            RetentionCohort {
                month: month("2026-10-01"),
                guilds: 2,
                week_eligible: 1,
                week_created: 1,
                month_eligible: 0,
                month_created: 0,
                active: 2,
            },
            RetentionCohort {
                month: month("2026-09-01"),
                guilds: 5,
                week_eligible: 5,
                week_created: 1,
                month_eligible: 5,
                month_created: 2,
                active: 1,
            },
        ]
    );
    assert_eq!(retention.unknown_joined, 1);
    assert_eq!(retention.unknown_joined_active, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn series_creation_counts_as_the_first_event(pool: PgPool) {
    // 繰り返し予定は初期シリーズ (event_series.is_creation) の作成日時で数える
    sqlx::raw_sql(
        r#"
        INSERT INTO guilds (guild_id, name, joined_at) VALUES ('1', 'g1', '2026-09-01T09:00:00');
        INSERT INTO event_series (guild_id, template, recurrence, rrule, start_at, end_at,
                                  created_at, created_by) VALUES
            ('1', '{}'::jsonb, '{}'::jsonb, 'FREQ=WEEKLY', '2026-09-10T10:00:00',
             '2026-09-10T11:00:00', '2026-09-03T09:00:00', 'u1');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let retention = guild_retention::guild_retention(&pool, now(), 30)
        .await
        .unwrap();
    let september = &retention.cohorts[0];
    assert_eq!(september.month, month("2026-09-01"));
    assert_eq!((september.week_created, september.month_created), (1, 1));
}

#[sqlx::test(migrations = "./migrations")]
async fn no_guilds_means_no_cohorts(pool: PgPool) {
    let retention = guild_retention::guild_retention(&pool, now(), 30)
        .await
        .unwrap();
    assert!(retention.cohorts.is_empty());
    assert_eq!(retention.unknown_joined, 0);
}
