//! 利用者の日次アクティビティ (#81) の記録の DB テスト。
//! `#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する (Postgres が必要)。
//! テスト DB には Better Auth の `user` テーブルが無いのでマイグレーションは外部キーを張らず、
//! 任意の user_id で行を入れられる (本番では `user` があるので外部キーが張られる)。

use chrono::NaiveDate;
use discalendar_api::models::user_activity;
use sqlx::PgPool;

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn record_keeps_one_row_per_user_and_day(pool: PgPool) {
    user_activity::record(&pool, "u1", date("2026-08-28"))
        .await
        .unwrap();
    // 同じ日の 2 回目はエラーにならず、行も増えない
    user_activity::record(&pool, "u1", date("2026-08-28"))
        .await
        .unwrap();
    user_activity::record(&pool, "u1", date("2026-08-29"))
        .await
        .unwrap();
    user_activity::record(&pool, "u2", date("2026-08-28"))
        .await
        .unwrap();

    let rows =
        sqlx::query!(r#"SELECT user_id, day FROM user_daily_activity ORDER BY user_id, day"#)
            .fetch_all(&pool)
            .await
            .unwrap();
    let rows: Vec<(String, NaiveDate)> = rows.into_iter().map(|r| (r.user_id, r.day)).collect();
    assert_eq!(
        rows,
        vec![
            ("u1".to_owned(), date("2026-08-28")),
            ("u1".to_owned(), date("2026-08-29")),
            ("u2".to_owned(), date("2026-08-28")),
        ]
    );
}
