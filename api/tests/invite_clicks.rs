//! 招待リンクの導入元ごとのクリック数 (#374) の DB テスト。
//! `#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する (Postgres が必要)。

use chrono::NaiveDate;
use discalendar_api::models::invite_clicks;
use sqlx::PgPool;

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

#[test]
fn normalize_keeps_known_sources_only() {
    assert_eq!(invite_clicks::normalize(Some("lp-hero")), "lp-hero");
    assert_eq!(invite_clicks::normalize(Some(" topgg ")), "topgg");
    assert_eq!(invite_clicks::normalize(None), "direct");
    assert_eq!(invite_clicks::normalize(Some("")), "direct");
    // 知らない値・大文字違い・長い文字列は記録しない
    assert_eq!(invite_clicks::normalize(Some("LP-HERO")), "other");
    assert_eq!(invite_clicks::normalize(Some("unknown")), "other");
    assert_eq!(invite_clicks::normalize(Some(&"a".repeat(1000))), "other");
}

#[sqlx::test]
async fn record_counts_per_day_and_source(pool: PgPool) {
    let today = date("2026-11-01");
    for _ in 0..3 {
        invite_clicks::record(&pool, "lp-hero", today)
            .await
            .unwrap();
    }
    invite_clicks::record(&pool, "lp-hero", date("2026-10-31"))
        .await
        .unwrap();
    invite_clicks::record(&pool, "docs-start", today)
        .await
        .unwrap();

    let rows: Vec<(NaiveDate, String, i64)> =
        sqlx::query_as("SELECT day, source, clicks FROM invite_clicks ORDER BY day, source")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        rows,
        vec![
            (date("2026-10-31"), "lp-hero".to_owned(), 1),
            (date("2026-11-01"), "docs-start".to_owned(), 1),
            (date("2026-11-01"), "lp-hero".to_owned(), 3),
        ]
    );
}

#[sqlx::test]
async fn summary_splits_recent_and_previous_windows(pool: PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO invite_clicks (day, source, clicks) VALUES
            -- 直近 30 日 (10-03〜11-01) の両端
            ('2026-11-01', 'lp-hero', 5),
            ('2026-10-03', 'lp-hero', 2),
            -- その前の 30 日 (09-03〜10-02) の両端
            ('2026-10-02', 'lp-hero', 7),
            ('2026-09-03', 'docs-start', 4),
            -- どちらにも入らない古い記録 (合計には入る)
            ('2026-09-02', 'docs-start', 1),
            ('2026-10-20', 'topgg', 7),
            -- 基準日より後 (数えない)
            ('2026-11-02', 'lp-hero', 100);
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let summary = invite_clicks::summary(&pool, date("2026-11-01"), 30)
        .await
        .unwrap();
    assert_eq!(summary.since, Some(date("2026-09-02")));
    let got: Vec<(&str, i64, i64, i64)> = summary
        .sources
        .iter()
        .map(|s| (s.source.as_str(), s.recent, s.previous, s.total))
        .collect();
    assert_eq!(
        got,
        vec![
            // 直近が同数 (7) なら合計の多い順
            ("lp-hero", 7, 7, 14),
            ("topgg", 7, 0, 7),
            ("docs-start", 0, 4, 5),
        ]
    );
}

#[sqlx::test]
async fn summary_is_empty_without_records(pool: PgPool) {
    let summary = invite_clicks::summary(&pool, date("2026-11-01"), 30)
        .await
        .unwrap();
    assert_eq!(summary.since, None);
    assert!(summary.sources.is_empty());
}
