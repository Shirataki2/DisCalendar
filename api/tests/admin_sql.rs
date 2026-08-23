//! SQL コンソール (#36) と定型操作の統合テスト。`#[sqlx::test]` がテストごとに一時 DB を作って
//! `migrations/` を適用する (Postgres が必要)。Better Auth のテーブル (`session` など) はマイグレーションに
//! 含まれないので、必要なテストでは最小限の形で作る。

use std::time::Duration;

use chrono::NaiveDateTime;
use discalendar_api::models::{
    admin_ops,
    admin_sql::{self, CELL_TRUNCATED_MARK, MAX_CELL_CHARS, MAX_ROWS, SqlError, SqlResult},
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

/// 起動時と同じくロールを用意し、そのロールでログインするプールを作る。
/// ロール (クラスタ共通) のパスワード設定はプロセスで 1 回だけにし、テスト DB ごとの権限付与は毎回行う
async fn console(pool: &PgPool) -> PgPool {
    static ROLE_READY: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    let password = ROLE_READY
        .get_or_init(|| async {
            let password = admin_sql::derive_password("test-secret");
            admin_sql::setup_role(pool, Some(&password)).await.unwrap();
            password
        })
        .await;
    admin_sql::setup_role(pool, None).await.unwrap();
    admin_sql::console_pool(&pool.connect_options(), password)
}

async fn run(pool: &PgPool, sql: &str) -> Result<SqlResult, SqlError> {
    admin_sql::execute(&console(pool).await, sql, TIMEOUT).await
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
    // 値は text にキャストした表現 (psql とほぼ同じ。boolean は true / false)。NULL は None
    assert_eq!(
        result.rows[0],
        vec![
            Some(created.id.to_string()),
            Some("meeting".to_owned()),
            None,
            Some("false".to_owned()),
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
    let console = console(&pool).await;
    let error = admin_sql::execute(&console, "SELECT pg_sleep(5)", Duration::from_millis(200))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, SqlError::Query(message) if message.contains("statement timeout")),
        "{error:?}"
    );
    // 打ち切られた後も接続は使える
    admin_sql::execute(&console, "SELECT 1", TIMEOUT)
        .await
        .unwrap();

    // 締切は 1 回の実行全体にかかる (FETCH ごとにリセットされない)。
    // 100 行ごとの FETCH が 0.4 秒ずつかかる文を 1 秒の締切で実行すると、3 回目の FETCH で打ち切られる
    let started = std::time::Instant::now();
    let error = admin_sql::execute(
        &console,
        "SELECT pg_sleep(0.004) FROM generate_series(1, 600)",
        Duration::from_secs(1),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&error, SqlError::Query(message) if message.contains("statement timeout")),
        "{error:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
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

    // SELECT 系の見た目でも書き込みを含めば Postgres が拒否する (データ変更を含む WITH はカーソルにできない。
    // シーケンス操作は専用ロールに権限が無い。それ以外は READ ONLY トランザクションが止める)
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
                if message.contains("read-only")
                    || message.contains("data-modifying")
                    || message.contains("permission denied")),
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
        // 専用ロールには権限が無いので EXPLAIN の時点で permission denied になる。
        // (権限が誤って付いていても計画の走査が「秘密情報」として拒否する)
        assert!(
            matches!(&error, SqlError::Rejected(m) | SqlError::Query(m)
                if m.contains("秘密情報") || m.contains("permission denied")),
            "{sql:?}: {error:?}"
        );
        assert!(!message.contains(SECRET), "{sql:?} leaked the secret");
    }

    // api の接続ロールに戻そうとしても、接続自体が専用ロールでログインしているので戻れない
    let api_user: String = sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    for sql in [
        "SELECT set_config('role', 'none', true), query_to_xml('SELECT token FROM session', true, false, '')".to_owned(),
        "SELECT CASE WHEN set_config('role', 'none', true) = 'none' THEN query_to_xml('SELECT token FROM session', true, false, '') END".to_owned(),
        format!("SELECT set_config('role', '{api_user}', true)"),
        format!("SELECT set_config('session_authorization', '{api_user}', true)"),
    ] {
        let error = run(&pool, &sql).await.unwrap_err();
        assert!(
            matches!(&error, SqlError::Query(m) if m.contains("permission denied")),
            "{sql:?}: {error:?}"
        );
        assert!(!error.to_string().contains(SECRET), "{sql:?} leaked the secret");
    }

    // 実行計画に表が現れない経路 (関数の内部で実行される SQL) は、専用ロールに権限が無いので Postgres が拒否する。
    // 値はエラーメッセージにも出ない
    for sql in [
        "SELECT table_to_xml('account', true, false, '')",
        "SELECT table_to_xml('public.session', true, false, '')",
        "SELECT query_to_xml('SELECT token FROM session', true, false, '')",
        "SELECT query_to_xml('SELECT value FROM verification', true, false, '')",
    ] {
        let error = run(&pool, sql).await.unwrap_err();
        assert!(
            matches!(&error, SqlError::Query(m) if m.contains("permission denied")),
            "{sql:?}: {error:?}"
        );
        assert!(
            !error.to_string().contains(SECRET),
            "{sql:?} leaked the secret"
        );
    }

    // superuser 専用の機能も使えない (api の接続ユーザーが superuser でも、コンソールのロールは違う)
    for sql in [
        "SELECT pg_read_file('/etc/hosts')",
        "SELECT pg_ls_dir('.')",
        "SELECT * FROM pg_statistic",
    ] {
        let error = run(&pool, sql).await.unwrap_err();
        assert!(
            matches!(&error, SqlError::Query(m) | SqlError::Rejected(m)
                if m.contains("permission denied") || m.contains("秘密情報")),
            "{sql:?}: {error:?}"
        );
    }

    // 保護テーブルを読まない文は通る (同名の文字列や別名は関係ない)。
    // schema_to_xml は権限のある表だけを出すので、保護テーブルは含まれない
    for sql in [
        "SELECT 'session' AS session, 'account' AS account",
        "SELECT schema_to_xml('public', true, false, '')",
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
async fn console_sessions_do_not_expose_their_queries(pool: PgPool) {
    // ロールに track_activities = off が設定され、pg_stat_activity.query に実行中の SQL が載らない
    let setting = run(&pool, "SELECT current_setting('track_activities')")
        .await
        .unwrap();
    assert_eq!(setting.rows[0][0].as_deref(), Some("off"));
    let shown = run(
        &pool,
        "SELECT query FROM pg_stat_activity WHERE pid = pg_backend_pid()",
    )
    .await
    .unwrap();
    // 収集されていないので空 (または "<command string not enabled>") になり、文そのものは見えない
    let query = shown.rows[0][0].clone().unwrap_or_default();
    assert!(
        query.is_empty() || query == "<command string not enabled>",
        "{query:?}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn known_words_cover_keywords_catalog_and_schema(pool: PgPool) {
    let words = admin_sql::load_known_words(&pool).await.unwrap();
    for w in [
        "select",
        "from",
        "guilds",
        "guild_id",
        "events",
        "start_at",
        "admin_audit_logs",
        "count",
        "generate_series",
        "text",
        "timestamptz",
        "pg_class",
        "relname",
        "server_version",
        "public",
    ] {
        assert!(words.contains(w), "{w} should be known");
    }
    assert!(!words.contains("gxicbvhtf38uoqojox9rlhazkjli0zu6"));
}

#[sqlx::test(migrations = "./migrations")]
async fn session_state_does_not_leak_into_the_pool(pool: PgPool) {
    let console = console(&pool).await;
    admin_sql::execute(
        &console,
        "SELECT pg_advisory_lock(8612004), pg_try_advisory_lock(424242), 'secret-literal-in-sql'",
        TIMEOUT,
    )
    .await
    .unwrap();
    // 同じプールの次の実行から、前の実行のプリペアドステートメント (SQL 全文) は見えない
    let prepared = admin_sql::execute(
        &console,
        "SELECT statement FROM pg_prepared_statements",
        TIMEOUT,
    )
    .await
    .unwrap();
    let dumped = format!("{:?}", prepared.rows);
    assert!(!dumped.contains("secret-literal-in-sql"), "{dumped}");
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' AND objid IN (8612004, 424242)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(held, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn cells_and_total_size_are_bounded(pool: PgPool) {
    // 1 セルはサーバー側で切り詰められ、印が付く
    let result = run(&pool, "SELECT repeat('x', 1000000) AS big, 'ok' AS small")
        .await
        .unwrap();
    let big = result.rows[0][0].as_deref().unwrap();
    assert_eq!(
        big.chars().count(),
        MAX_CELL_CHARS + CELL_TRUNCATED_MARK.chars().count()
    );
    assert!(big.ends_with(CELL_TRUNCATED_MARK));
    assert_eq!(result.rows[0][1].as_deref(), Some("ok"));
    // 列名と型名は元の列のもの
    assert_eq!(result.columns[0].name, "big");
    assert_eq!(result.columns[0].type_name, "TEXT");

    // 合計バイト数の上限で行数上限より前に打ち切られる (10 列 × 4000 文字 = 40KB/行)
    let cols: Vec<String> = (0..10)
        .map(|i| format!("repeat('y', 4000) AS c{i}"))
        .collect();
    let sql = format!("SELECT {} FROM generate_series(1, 1000)", cols.join(", "));
    let result = run(&pool, &sql).await.unwrap();
    assert!(result.truncated);
    assert!(result.row_count < MAX_ROWS, "{}", result.row_count);
    assert!(result.row_count > 50, "{}", result.row_count);
    let total: usize = result
        .rows
        .iter()
        .flatten()
        .flatten()
        .map(String::len)
        .sum();
    assert!(total <= admin_sql::MAX_RESULT_BYTES, "{total}");

    // 列が非常に多い 1 行 (1,500 列 × 4,001 文字 ≈ 6MB) でも 4 MiB を超える行は返さず truncated になる
    let many = vec!["q.c"; 1500];
    let sql = format!(
        "SELECT {} FROM (SELECT repeat('z', 5000) AS c) AS q, generate_series(1, 3)",
        many.join(", ")
    );
    let result = run(&pool, &sql).await.unwrap();
    assert_eq!(result.columns.len(), 1500);
    assert!(result.truncated);
    assert!(result.rows.is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn wrapping_keeps_odd_but_valid_statements_working(pool: PgPool) {
    // 同名の列、末尾の `;`、末尾行のコメント、列の無い SELECT、ORDER BY
    let dup = run(&pool, "SELECT 1 AS a, 2 AS a;").await.unwrap();
    assert_eq!(
        dup.columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["a", "a"]
    );
    assert_eq!(
        dup.rows[0],
        vec![Some("1".to_owned()), Some("2".to_owned())]
    );

    let commented = run(
        &pool,
        "SELECT 1 AS n -- trailing comment
;
-- last line comment",
    )
    .await
    .unwrap();
    assert_eq!(commented.rows[0][0].as_deref(), Some("1"));

    let no_columns = run(&pool, "SELECT FROM guilds").await.unwrap();
    assert!(no_columns.columns.is_empty());

    let ordered = run(
        &pool,
        "SELECT n FROM generate_series(1, 3) AS n ORDER BY n DESC",
    )
    .await
    .unwrap();
    let values: Vec<&str> = ordered
        .rows
        .iter()
        .map(|r| r[0].as_deref().unwrap())
        .collect();
    assert_eq!(values, ["3", "2", "1"]);

    // bytea / json / 配列 / timestamptz も psql と同じテキスト表現
    let typed = run(
        &pool,
        r#"SELECT '\xdead'::bytea, '{"a": 1}'::jsonb, ARRAY[1, 2], TIMESTAMPTZ '2026-08-23 10:00:00+09'"#,
    )
    .await
    .unwrap();
    assert_eq!(typed.rows[0][0].as_deref(), Some("\\xdead"));
    assert_eq!(typed.rows[0][1].as_deref(), Some(r#"{"a": 1}"#));
    assert_eq!(typed.rows[0][2].as_deref(), Some("{1,2}"));
    assert_eq!(typed.columns[3].type_name, "TIMESTAMPTZ");
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
    let (snapshot, deleted) = admin_ops::delete_guild_events(&mut tx, GUILD)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(deleted, 2);
    let names: Vec<&str> = snapshot.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);

    let remaining = run(&pool, "SELECT guild_id, name FROM events")
        .await
        .unwrap();
    assert_eq!(
        remaining.rows,
        vec![vec![Some(OTHER_GUILD.to_owned()), Some("c".to_owned())]]
    );

    // 予定が無いギルドは 0 件
    let mut tx = pool.begin().await.unwrap();
    let (snapshot, deleted) = admin_ops::delete_guild_events(&mut tx, GUILD)
        .await
        .unwrap();
    assert!(snapshot.is_empty());
    assert_eq!(deleted, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn delete_guild_events_snapshot_is_bounded(pool: PgPool) {
    let n = admin_ops::DELETE_SNAPSHOT_LIMIT + 5;
    sqlx::query(
        "INSERT INTO events (guild_id, name, notifications, color, is_all_day, start_at, end_at, created_at)
         SELECT $1, 'e' || g, '{}', '#000000', false, now(), now(), now() FROM generate_series(1, $2) g",
    )
    .bind(GUILD)
    .bind(n as i32)
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let (snapshot, deleted) = admin_ops::delete_guild_events(&mut tx, GUILD)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(deleted, n as u64);
    assert_eq!(snapshot.len(), admin_ops::DELETE_SNAPSHOT_LIMIT as usize);
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
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
