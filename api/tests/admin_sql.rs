//! SQL コンソール (#36) と定型操作の統合テスト。`#[sqlx::test]` がテストごとに一時 DB を作って
//! `migrations/` を適用する (Postgres が必要)。Better Auth のテーブル (`session` など) はマイグレーションに
//! 含まれないので、必要なテストでは最小限の形で作る。

use std::time::Duration;

use chrono::NaiveDateTime;
use discalendar_api::models::{
    admin_ops,
    admin_sql::{self, MAX_ROWS, SqlError, SqlResult},
    events::{self, EventInput},
};
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";
const TIMEOUT: Duration = Duration::from_secs(10);
/// 漏れてはいけない値。結果やエラーメッセージに一切現れないことを確認する
const SECRET: &str = "secret-token-value-do-not-leak";

fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}

fn input(name: &str) -> EventInput {
    EventInput {
        name: name.to_owned(),
        description: None,
        notifications: vec![],
        color: "#2196F3".to_owned(),
        is_all_day: false,
        start_at: dt("2026-08-22T10:00:00"),
        end_at: dt("2026-08-22T11:00:00"),
    }
}

async fn run(pool: &PgPool, sql: &str) -> Result<SqlResult, SqlError> {
    admin_sql::execute(pool, sql, TIMEOUT).await
}

/// Better Auth のテーブルをテスト DB に用意し、秘密の値を入れる
async fn create_auth_tables(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        CREATE TABLE "user" (id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL);
        CREATE TABLE "session" (
            id TEXT PRIMARY KEY, token TEXT NOT NULL, "expiresAt" TIMESTAMPTZ NOT NULL, "userId" TEXT NOT NULL
        );
        CREATE TABLE "account" (
            id TEXT PRIMARY KEY, "accountId" TEXT NOT NULL, "providerId" TEXT NOT NULL, "userId" TEXT NOT NULL,
            "accessToken" TEXT, "refreshToken" TEXT
        );
        CREATE TABLE "verification" (id TEXT PRIMARY KEY, identifier TEXT NOT NULL, value TEXT NOT NULL);
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO "user" (id, name, email) VALUES ('u1', 'alice', 'alice@example.com')"#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO "session" (id, token, "expiresAt", "userId") VALUES ('s1', $1, now() + interval '1 day', 'u1')"#,
    )
    .bind(SECRET)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO "account" (id, "accountId", "providerId", "userId", "accessToken", "refreshToken")
           VALUES ('a1', '123456789012345678', 'discord', 'u1', $1, $1)"#,
    )
    .bind(SECRET)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(r#"INSERT INTO "verification" (id, identifier, value) VALUES ('v1', 'x', $1)"#)
        .bind(SECRET)
        .execute(pool)
        .await
        .unwrap();
    // プランナ統計 (pg_stats の histogram_bounds) に実値が載る状態にしておく
    sqlx::query(
        r#"INSERT INTO "session" (id, token, "expiresAt", "userId")
           SELECT 'bulk' || n, $1 || n, now() + interval '1 day', 'u1' FROM generate_series(1, 200) AS n"#,
    )
    .bind(SECRET)
    .execute(pool)
    .await
    .unwrap();
    sqlx::raw_sql(r#"ANALYZE "session""#)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn select_returns_columns_and_text_values(pool: PgPool) {
    let created = events::create(&pool, GUILD, &input("meeting"), dt("2026-08-01T00:00:00"))
        .await
        .unwrap();

    let result = run(
        &pool,
        "SELECT id, name, description, is_all_day, start_at, notifications FROM events ORDER BY id",
    )
    .await
    .unwrap();
    let names: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "id",
            "name",
            "description",
            "is_all_day",
            "start_at",
            "notifications"
        ]
    );
    assert_eq!(result.columns[0].type_name, "INT4");
    assert_eq!(result.columns[3].type_name, "BOOL");
    assert_eq!(result.row_count, 1);
    assert!(!result.truncated);
    // 値は psql と同じテキスト表現。NULL は None
    assert_eq!(
        result.rows[0],
        vec![
            Some(created.id.to_string()),
            Some("meeting".to_owned()),
            None,
            Some("f".to_owned()),
            Some("2026-08-22 10:00:00".to_owned()),
            Some("{}".to_owned()),
        ]
    );

    // 0 行でもカラム名は返る
    let empty = run(&pool, "SELECT guild_id, name FROM guilds WHERE false")
        .await
        .unwrap();
    assert_eq!(empty.columns.len(), 2);
    assert_eq!(empty.columns[0].name, "guild_id");
    assert!(empty.rows.is_empty());

    // 括弧始まりの SELECT、WITH、VALUES、TABLE も通る
    for sql in [
        "(SELECT 1 AS n) UNION ALL (SELECT 2)",
        "WITH x AS (SELECT 1 AS n) SELECT n FROM x",
        "VALUES (1, 'a'), (2, 'b')",
        "TABLE guilds",
        "-- comment first\nSELECT 1",
    ] {
        run(&pool, sql)
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn row_limit_truncates_large_results(pool: PgPool) {
    let result = run(&pool, "SELECT generate_series(1, 100000) AS n")
        .await
        .unwrap();
    assert_eq!(result.row_count, MAX_ROWS);
    assert_eq!(result.rows.len(), MAX_ROWS);
    assert!(result.truncated);
    assert_eq!(result.rows[0][0].as_deref(), Some("1"));
    assert_eq!(result.rows[MAX_ROWS - 1][0].as_deref(), Some("500"));

    // ちょうど上限なら truncated にならない
    let exact = run(&pool, &format!("SELECT generate_series(1, {MAX_ROWS})"))
        .await
        .unwrap();
    assert_eq!(exact.row_count, MAX_ROWS);
    assert!(!exact.truncated);
}

#[sqlx::test(migrations = "./migrations")]
async fn statement_timeout_cancels_long_queries(pool: PgPool) {
    let error = admin_sql::execute(&pool, "SELECT pg_sleep(5)", Duration::from_millis(200))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, SqlError::Query(message) if message.contains("statement timeout")),
        "{error:?}"
    );
    // 打ち切られた後も接続は使える
    run(&pool, "SELECT 1").await.unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn writes_and_non_read_only_statements_are_rejected(pool: PgPool) {
    events::create(&pool, GUILD, &input("keep"), dt("2026-08-01T00:00:00"))
        .await
        .unwrap();

    // 先頭キーワードで弾くもの (実行されない)
    for sql in [
        "INSERT INTO events (guild_id, name, color, is_all_day, start_at, end_at, created_at) VALUES ('1', 'x', '#fff', false, now(), now(), now())",
        "UPDATE events SET name = 'x'",
        "DELETE FROM events",
        "TRUNCATE events",
        "DROP TABLE events",
        "CREATE TEMP TABLE t AS SELECT 1",
        "SET statement_timeout = 0",
        "RESET statement_timeout",
        "DO $$ BEGIN PERFORM 1; END $$",
        "COPY events TO STDOUT",
        "BEGIN",
        "COMMIT",
        "ROLLBACK",
        "LOCK TABLE events IN ACCESS EXCLUSIVE MODE",
        "VACUUM events",
        "ANALYZE events",
        "LISTEN foo",
        "NOTIFY foo",
        "PREPARE p AS SELECT 1",
        "DECLARE c CURSOR FOR SELECT 1",
        "FETCH ALL FROM admin_console_cursor",
        "",
    ] {
        let error = run(&pool, sql).await.unwrap_err();
        assert!(matches!(error, SqlError::Rejected(_)), "{sql:?}: {error:?}");
    }

    // 複数文はプリペアドステートメントにできないので失敗する (SET で statement_timeout を外す等ができない)
    let error = run(&pool, "SELECT 1; SET statement_timeout = 0")
        .await
        .unwrap_err();
    assert!(
        matches!(&error, SqlError::Rejected(message) if message.contains("複数の文")),
        "{error:?}"
    );

    // SELECT 系の見た目でも書き込みを含めば Postgres が拒否する
    // (データ変更を含む WITH はカーソルにできない。それ以外は READ ONLY トランザクションが止める)
    for sql in [
        "WITH d AS (DELETE FROM events RETURNING id) SELECT * FROM d",
        "WITH i AS (INSERT INTO guild_config (guild_id, restricted) VALUES ('1', true) RETURNING guild_id) SELECT * FROM i",
        "SELECT * FROM events FOR UPDATE",
        "SELECT nextval('events_id_seq')",
        "SELECT setval('events_id_seq', 1)",
    ] {
        let error = run(&pool, sql).await.unwrap_err();
        assert!(
            matches!(&error, SqlError::Query(message)
                if message.contains("read-only") || message.contains("data-modifying")),
            "{sql:?}: {error:?}"
        );
    }

    // 何も消えていない
    let rows = events::list_between(
        &pool,
        GUILD,
        dt("2026-08-01T00:00:00"),
        dt("2026-09-01T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);

    // Postgres の構文エラーはメッセージ付きで返る
    let error = run(&pool, "SELECT * FROM").await.unwrap_err();
    assert!(
        matches!(&error, SqlError::Query(message) if message.contains("syntax error")),
        "{error:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn protected_tables_are_rejected_before_execution(pool: PgPool) {
    create_auth_tables(&pool).await;

    // どう書いても実行計画に Relation Name が現れるので、すべて実行前に拒否される
    for sql in [
        "SELECT token FROM session",
        r#"SELECT * FROM "session""#,
        "SELECT s.* FROM public.session AS s",
        "select TOKEN from SESSION",
        "SELECT (SELECT token FROM session LIMIT 1) AS t",
        "WITH s AS (SELECT * FROM session) SELECT * FROM s",
        "SELECT * FROM events e LEFT JOIN session s ON true",
        "SELECT e.id FROM events e WHERE EXISTS (SELECT 1 FROM session)",
        "TABLE account",
        "SELECT count(*) FROM verification",
        r#"SELECT * FROM U&"s\0065ssion""#,
        "SELECT 1 UNION ALL SELECT 1 FROM account",
        "VALUES ((SELECT value FROM verification LIMIT 1))",
        // 実行すればエラーメッセージに値が載るが、実行前に拒否される
        "SELECT token::int FROM session",
        "EXPLAIN ANALYZE SELECT token::int FROM session",
        "EXPLAIN (ANALYZE, FORMAT JSON) SELECT * FROM account",
        "explain analyze verbose select \"refreshToken\"::int from account",
        // プランナ統計には列のサンプル値 (実際のトークン) が入る
        "SELECT histogram_bounds, most_common_vals FROM pg_stats WHERE tablename = 'session'",
        "SELECT * FROM pg_statistic",
        "SELECT * FROM pg_stats_ext",
    ] {
        let error = run(&pool, sql).await.unwrap_err();
        let message = error.to_string();
        assert!(
            matches!(&error, SqlError::Rejected(m) if m.contains("秘密情報")),
            "{sql:?}: {error:?}"
        );
        assert!(!message.contains(SECRET), "{sql:?} leaked the secret");
    }

    // 保護テーブルを読まない文は通る (同名の文字列や別名は関係ない)
    for sql in [
        "SELECT 'session' AS session, 'account' AS account",
        r#"SELECT id, name, email FROM "user""#,
        "SELECT * FROM events",
        "SELECT count(*) FROM admin_audit_logs",
        "SELECT relname FROM pg_class WHERE relname = 'session'",
        "EXPLAIN SELECT * FROM events",
    ] {
        let result = run(&pool, sql)
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
        let dumped = format!("{:?}", result.rows);
        assert!(!dumped.contains(SECRET), "{sql:?} leaked the secret");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn explain_and_show_work(pool: PgPool) {
    let plan = run(&pool, "EXPLAIN SELECT * FROM events WHERE guild_id = '1'")
        .await
        .unwrap();
    assert_eq!(plan.columns[0].name, "QUERY PLAN");
    assert!(!plan.rows.is_empty());

    let analyzed = run(
        &pool,
        "EXPLAIN (ANALYZE, FORMAT JSON) SELECT count(*) FROM events",
    )
    .await
    .unwrap();
    assert_eq!(analyzed.row_count, 1);

    let version = run(&pool, "SHOW server_version").await.unwrap();
    assert_eq!(version.columns[0].name, "server_version");
    assert_eq!(version.row_count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_guild_events_removes_only_that_guild(pool: PgPool) {
    for name in ["a", "b"] {
        events::create(&pool, GUILD, &input(name), dt("2026-08-01T00:00:00"))
            .await
            .unwrap();
    }
    events::create(&pool, OTHER_GUILD, &input("c"), dt("2026-08-01T00:00:00"))
        .await
        .unwrap();

    let mut tx = pool.begin().await.unwrap();
    let deleted = admin_ops::delete_guild_events(&mut *tx, GUILD)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut names: Vec<&str> = deleted.iter().map(|e| e.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["a", "b"]);

    let remaining = run(&pool, "SELECT guild_id, name FROM events")
        .await
        .unwrap();
    assert_eq!(
        remaining.rows,
        vec![vec![Some(OTHER_GUILD.to_owned()), Some("c".to_owned())]]
    );

    // 予定が無いギルドは 0 件
    assert!(
        admin_ops::delete_guild_events(&pool, GUILD)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn purge_expired_sessions_keeps_valid_ones(pool: PgPool) {
    create_auth_tables(&pool).await;
    sqlx::query(
        r#"INSERT INTO "session" (id, token, "expiresAt", "userId")
           VALUES ('old1', 'x', now() - interval '1 hour', 'u1'), ('old2', 'y', now() - interval '30 days', 'u1')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(admin_ops::purge_expired_sessions(&pool).await.unwrap(), 2);
    assert_eq!(admin_ops::purge_expired_sessions(&pool).await.unwrap(), 0);
    let remaining: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM "session""#)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 201);
}
