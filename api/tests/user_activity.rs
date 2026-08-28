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

async fn user_fk_exists(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM pg_constraint
            WHERE conname = 'user_daily_activity_user_id_fkey'
              AND conrelid = 'public.user_daily_activity'::regclass
        )
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 新規環境では api のマイグレーションが web (Better Auth のテーブル作成) より先に走るため、
/// migration 内の条件付き外部キーは張られない。"user" ができた後の起動で
/// `ensure_user_fk` が孤児を消してから張り直すこと (PR #113 のレビュー指摘)
#[sqlx::test(migrations = "./migrations")]
async fn ensure_user_fk_adds_the_constraint_once_user_table_exists(pool: PgPool) {
    let mut conn = pool.acquire().await.unwrap();

    // "user" がまだ無い間は何もしない (エラーにもならない)
    user_activity::ensure_user_fk(&mut conn).await.unwrap();
    assert!(!user_fk_exists(&pool).await);

    // web が Better Auth のテーブルを作り、外部キーが無い間に孤児 (user に居ない記録) ができた状況
    sqlx::raw_sql(
        r#"
        CREATE TABLE "user" (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL,
            "emailVerified" BOOLEAN NOT NULL DEFAULT false, image TEXT,
            "createdAt" TIMESTAMPTZ NOT NULL DEFAULT now(), "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT now()
        );
        INSERT INTO "user" (id, name, email) VALUES ('u1', 'Alice', 'alice@example.com');
        INSERT INTO user_daily_activity (user_id, day) VALUES
            ('u1', '2026-08-28'), ('ghost', '2026-08-28');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // 次の起動で外部キーが張られ、孤児は消える (残っていると制約の検証に失敗する)
    user_activity::ensure_user_fk(&mut conn).await.unwrap();
    assert!(user_fk_exists(&pool).await);
    let users: Vec<String> =
        sqlx::query_scalar("SELECT user_id FROM user_daily_activity ORDER BY user_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(users, vec!["u1".to_owned()]);

    // 以後の起動では何もしない (二重に張ろうとしてエラーにならない)
    user_activity::ensure_user_fk(&mut conn).await.unwrap();

    // ON DELETE CASCADE: 利用者に関する情報の削除で記録も消える
    sqlx::query(r#"DELETE FROM "user" WHERE id = 'u1'"#)
        .execute(&pool)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM user_daily_activity")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}
