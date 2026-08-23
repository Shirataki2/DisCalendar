//! 管理コンソールの読み取り専用 SQL コンソール (#36)。
//!
//! 任意の SQL を受け取るので、壊さない・漏らさないための層を重ねる:
//!
//! 1. 先頭のキーワードを `SELECT` / `WITH` / `VALUES` / `TABLE` / `EXPLAIN` / `SHOW` に限る
//!    (`SET` / `DO` / `COPY` / DDL / `BEGIN` などはここで弾く)
//! 2. **専用の DB ロールでログインした別の接続プールで実行する (P0)**: [`ROLE`] (`discalendar_sql_console`、
//!    superuser ではない) には `public` スキーマのテーブルの SELECT だけを与え、Better Auth の `account`
//!    (Discord の access / refresh token)、`session` (セッショントークン)、`verification` ([`PROTECTED_TABLES`]) は
//!    権限を外してあるので、`table_to_xml('account', ...)` や `query_to_xml(...)` のように関数の内部で実行される
//!    SQL でも読めない (`permission denied`)。superuser でないので `pg_read_file` や他の接続の
//!    `pg_terminate_backend` も使えず、プランナ統計 `pg_statistic` (列のサンプル値に実トークンが入る) も読めない。
//!    api の接続で `SET ROLE` するのではなくこのロール自身でログインするので、`set_config('role', 'none', true)` の
//!    ような SQL からのロール解除でも api のロールには戻れない。ロールの作成・パスワード設定・権限付与は
//!    api の起動時に [`setup_role`] が行い (パスワードは `BETTER_AUTH_SECRET` から導出)、できない環境では
//!    手で作って `SQL_CONSOLE_DATABASE_URL` で渡す (README)。実行のたびに接続のセッションユーザーが
//!    このロールで superuser でなく保護テーブルを読めないことを確かめ、満たさなければ実行しない (fail closed)
//! 3. `BEGIN READ ONLY` のトランザクションで実行する (`WITH ... DELETE` のような書き込みは Postgres が拒否し、
//!    ロールにも書き込み権限が無い) ので、稼働中の旧 Bot と共有している `events` などを誤って壊せない
//! 4. `SET LOCAL statement_timeout` で長いクエリを打ち切る。カーソルからの `FETCH` は複数回に分けるので、
//!    コマンドごとではなく開始時刻からの締切 ([`STATEMENT_TIMEOUT`]) で残り時間を設定し直す
//! 5. **実行計画での事前判定**: `EXPLAIN (FORMAT JSON)` の計画を走査し、`Relation Name` に保護テーブルや
//!    プランナ統計が出てくる文は実行前に拒否する (2 の多層防御。権限が誤って付いても直接の読み取りは止まる)。
//!    SQL の文字列を見て判定すると別名・サブクエリ・`U&"..."` の識別子などですり抜けられるが、
//!    計画に出てくる名前はどう書いても変わらない
//! 6. 文字列はプリペアドステートメントとして `prepare` する (カラム名・型の取得も兼ねる)。Postgres は
//!    1 つのプリペアドステートメントに複数の文を入れられないので、`SELECT 1; SET ...` のような複数文は
//!    ここ (と 5 の EXPLAIN) で失敗する
//! 7. 行数とサイズ: SELECT 系は `SELECT left(c1::text, N), ... FROM (<sql>) AS q(c1, ...)` に包んで
//!    `DECLARE ... CURSOR` にし、`FETCH` を小分けにして [`MAX_ROWS`] 行・[`MAX_RESULT_BYTES`] バイトで打ち切る。
//!    セルの切り詰め ([`MAX_CELL_CHARS`]) もサーバー側で行うので、巨大な値でサーバーから送られてくる量が膨らまない。
//!    1 回の FETCH の行数は列数から決めて (最悪でも [`MAX_FETCH_BATCH_BYTES`] 程度)、列が多くても 1 バッチが膨らまない
//! 8. 実行後は `pg_advisory_unlock_all()` で、セッションに残りうる advisory lock
//!    (`pg_advisory_lock(...)` はロールバックでは解放されない) を外してから接続をプールに返す。外せなければ接続を閉じる
//!
//! 結果の値は text にキャストして simple query protocol (text format) で受け取り、型ごとのデコードをせずに
//! 文字列表現のまま返す (psql とほぼ同じ。boolean は `true` / `false`。NULL は null)。
//!
//! 監査ログ・履歴に残す SQL は [`redact_sql`] で文字列リテラルとコメントを伏せる (管理者が調査中に貼り付けた
//! cookie やトークンが履歴として永続化・他の管理者に表示されないように)。

use std::time::{Duration, Instant};

use hmac::{Hmac, KeyInit as _, Mac as _};
use serde::Serialize;
use sha2::Sha256;
use sqlx::{
    AssertSqlSafe, Column as _, Connection as _, Executor as _, PgConnection, PgPool, Row as _,
    SqlSafeStr as _, Statement as _, TypeInfo as _, ValueRef as _,
    pool::PoolConnection,
    postgres::{PgConnectOptions, PgPoolOptions, Postgres},
};
use utoipa::ToSchema;

/// 1 回の実行で返す最大行数。これを超えた分は捨てて `truncated` を立てる
pub const MAX_ROWS: usize = 500;
/// 1 回の実行で返す値の合計バイト数の上限 (超えたらそこで打ち切って `truncated`)
pub const MAX_RESULT_BYTES: usize = 4 * 1024 * 1024;
/// 1 セルの文字数の上限。超えた分はサーバー側で切り捨て、末尾に [`CELL_TRUNCATED_MARK`] を付ける
pub const MAX_CELL_CHARS: usize = 4_000;
/// 切り詰めたセルの末尾に付ける印
pub const CELL_TRUNCATED_MARK: &str = "…(切り詰め)";
/// カーソルから 1 回に取り出す行数の上限 (列数が多いときは [`fetch_batch_rows`] で減らす)
const MAX_FETCH_BATCH: usize = 100;
/// 1 回の FETCH で受け取る最悪サイズの目安 (行数 × 列数 × セル上限がこれを超えないように行数を決める)
const MAX_FETCH_BATCH_BYTES: usize = 8 * 1024 * 1024;
/// 1 回の実行 (計画の確認からすべての FETCH まで) に使える時間
pub const STATEMENT_TIMEOUT: Duration = Duration::from_secs(10);
/// 受け付ける SQL の長さ (文字数) の上限。監査ログにもそのまま残す
pub const MAX_SQL_CHARS: usize = 10_000;
/// SQL コンソールの実行に使う DB ロール (superuser ではない。[`setup_role`] が作る)
pub const ROLE: &str = "discalendar_sql_console";
/// コンソール用プールの接続数 (管理者が同時に使う数は少ない)
const POOL_SIZE: u32 = 2;
/// 読み取りを禁止するテーブル (Better Auth のトークン類を持つ)。このロールには権限を与えず、
/// さらに実行計画にこれらが出てくる文は実行前に拒否する (スキーマに関係なく名前で判定)
pub const PROTECTED_TABLES: [&str; 3] = ["account", "session", "verification"];
/// 実行計画で見つけたら拒否する名前 ([`PROTECTED_TABLES`] + プランナ統計。`pg_stats` 系はビューなので
/// 計画上は `pg_statistic*` として現れるが、念のため名前も入れる)
const REJECTED_RELATIONS: [&str; 8] = [
    "account",
    "session",
    "verification",
    "pg_statistic",
    "pg_statistic_ext_data",
    "pg_stats",
    "pg_stats_ext",
    "pg_stats_ext_exprs",
];

/// 実行結果
#[derive(Debug, Serialize, ToSchema)]
pub struct SqlResult {
    pub columns: Vec<SqlColumn>,
    /// 行ごとの値 (カラム順)。値は Postgres のテキスト表現、NULL は null。
    /// `MAX_CELL_CHARS` を超える値は切り詰めて末尾に `CELL_TRUNCATED_MARK` が付く
    #[schema(value_type = Vec<Vec<Option<String>>>)]
    pub rows: Vec<Vec<Option<String>>>,
    /// 返した行数 (`rows.len()`)
    pub row_count: usize,
    /// `MAX_ROWS` 行か `MAX_RESULT_BYTES` バイトを超えたので打ち切ったか
    pub truncated: bool,
    /// 実行にかかった時間 (ミリ秒)
    pub duration_ms: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SqlColumn {
    pub name: String,
    /// Postgres の型名 (`INT4`, `TEXT`, `TIMESTAMPTZ` など)
    #[serde(rename = "type")]
    pub type_name: String,
}

/// 実行できなかった理由
#[derive(Debug, thiserror::Error)]
pub enum SqlError {
    /// こちらの規則で拒否した (実行していない)
    #[error("{0}")]
    Rejected(String),
    /// Postgres がエラーを返した (構文エラー、権限エラー、読み取り専用違反、タイムアウトなど)。メッセージはそのまま見せてよい
    #[error("{0}")]
    Query(String),
    /// SQL コンソール用のロール / 接続が使えない (作られていない / 権限が正しくない)。実行していない
    #[error("{0}")]
    Unavailable(String),
    /// 接続や内部の失敗
    #[error(transparent)]
    Other(#[from] sqlx::Error),
}

impl SqlError {
    fn from_sqlx(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::Database(db) => {
                let code = db.code().map(|c| c.into_owned()).unwrap_or_default();
                let message = db.message().to_owned();
                match code.as_str() {
                    // query_canceled (statement_timeout)
                    "57014" => Self::Query(format!(
                        "{message} (上限 {} 秒)",
                        STATEMENT_TIMEOUT.as_secs()
                    )),
                    _ if message.contains("multiple commands") => {
                        Self::Rejected("複数の文は実行できません (1 文だけにしてください)".into())
                    }
                    _ => Self::Query(format!("{message} (SQLSTATE {code})")),
                }
            }
            // sqlx が列の型情報を解決できない (pg_catalog の内部型など)。接続の問題ではないので利用者に返す
            error @ (sqlx::Error::ColumnDecode { .. } | sqlx::Error::TypeNotFound { .. }) => {
                Self::Query(format!("結果の列の型を扱えません: {error}"))
            }
            other => Self::Other(other),
        }
    }

    fn timed_out() -> Self {
        Self::Query(format!(
            "canceling statement due to statement timeout (上限 {} 秒)",
            STATEMENT_TIMEOUT.as_secs()
        ))
    }
}

/// 文の種類 (先頭キーワードで判定)
#[derive(Debug, PartialEq, Eq)]
enum Statement<'a> {
    /// SELECT / WITH / VALUES / TABLE。サブクエリに包んでカーソルで行数を絞って実行する
    Select,
    /// EXPLAIN。`inner` は EXPLAIN の対象の文 (保護テーブルの判定にはこちらを使う)
    Explain { inner: &'a str },
    /// SHOW。対象のテーブルが無いので計画の確認は不要
    Show,
}

/// [`ROLE`] のパスワードを `BETTER_AUTH_SECRET` から導出する (HMAC-SHA256 の hex)。
/// 複数の api インスタンスが同じ値になり、新しい env を増やさずに済む。
/// この秘密が漏れた時点でセッションの偽造ができてしまうので、ここから導出しても守るものは増えも減りもしない
pub fn derive_password(better_auth_secret: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(better_auth_secret.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(b"discalendar_sql_console_password");
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// [`ROLE`] でログインするコンソール用のプール。`base` (api の `DATABASE_URL`) のホスト・DB はそのままに
/// ユーザーとパスワードだけ差し替える。接続は遅延 (最初の実行時)
pub fn console_pool(base: &PgConnectOptions, password: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(POOL_SIZE)
        .connect_lazy_with(base.clone().username(ROLE).password(password))
}

/// `SQL_CONSOLE_DATABASE_URL` で渡された接続文字列からコンソール用のプールを作る (手動でロールを用意する環境向け)
pub fn console_pool_from_url(url: &str) -> Result<PgPool, sqlx::Error> {
    let options: PgConnectOptions = url.parse()?;
    Ok(PgPoolOptions::new()
        .max_connections(POOL_SIZE)
        .connect_lazy_with(options))
}

/// SQL コンソール用のロール [`ROLE`] を用意する (api の起動時に呼ぶ。冪等)。
///
/// 1. ロールが無ければ `CREATE ROLE`、`password` があれば `ALTER ROLE ... LOGIN PASSWORD` で
///    ログインできるようにする (CREATEROLE か superuser が要る。compose の Postgres (`POSTGRES_USER`) は superuser)
/// 2. `public` スキーマの全テーブルに SELECT を与え、[`PROTECTED_TABLES`] (存在するもの) からは取り上げる
///    (api の接続ユーザーがテーブルの所有者であればよい)。Better Auth のテーブルは web が起動時に作るので、
///    api を先に起動した環境では次回の起動で付与される (それまでロールは読めない = 安全側)
///
/// 1 と 2 は別のトランザクションで、1 が権限不足で失敗しても 2 は試みる (手でロールを作った環境で、
/// 後から増えたテーブルの権限を api の再起動で追従させるため)。どちらかが失敗したら Err
pub async fn setup_role(pool: &PgPool, password: Option<&str>) -> anyhow::Result<()> {
    let role_result = ensure_role(pool, password).await;
    let grant_result = grant_privileges(pool).await;
    match (role_result, grant_result) {
        (Ok(()), Ok(())) => {
            tracing::info!(role = ROLE, "SQL console role is ready");
            Ok(())
        }
        (Err(e), Ok(())) | (Ok(()), Err(e)) => Err(e),
        (Err(role), Err(grant)) => Err(grant.context(format!("also: {role:#}"))),
    }
}

async fn ensure_role(pool: &PgPool, password: Option<&str>) -> anyhow::Result<()> {
    let role_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = $1)")
            .bind(ROLE)
            .fetch_one(pool)
            .await?;
    if !role_exists {
        // 同時に起動した別インスタンスと競合したら duplicate_object (42710) か pg_authid の一意制約違反 (23505)
        // になるので、それは成功扱い
        if let Err(error) = sqlx::raw_sql(AssertSqlSafe(format!("CREATE ROLE {ROLE} NOLOGIN")))
            .execute(pool)
            .await
        {
            let duplicate = matches!(&error, sqlx::Error::Database(db)
                if matches!(db.code().as_deref(), Some("42710" | "23505")));
            if !duplicate {
                return Err(anyhow::Error::new(error).context(format!(
                    "failed to create the role {ROLE} (CREATEROLE or superuser is required)"
                )));
            }
        }
    }
    if let Some(password) = password {
        // パスワードは derive_password の hex (英数字のみ) なのでそのまま埋め込める
        debug_assert!(password.chars().all(|c| c.is_ascii_hexdigit()));
        let statement = format!("ALTER ROLE {ROLE} LOGIN PASSWORD '{password}'");
        // 複数の api インスタンスが同時に起動すると pg_authid の同じ行を更新して
        // "tuple concurrently updated" (XX000) になることがあるので、少し待って再試行する
        let mut attempt = 0;
        loop {
            match sqlx::raw_sql(AssertSqlSafe(statement.clone()))
                .execute(pool)
                .await
            {
                Ok(_) => break,
                Err(sqlx::Error::Database(db))
                    if attempt < 5 && db.message().contains("tuple concurrently updated") =>
                {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(100 * attempt)).await;
                }
                Err(e) => {
                    return Err(anyhow::Error::new(e).context(format!(
                        "failed to set the password of {ROLE} (CREATEROLE or superuser is required)"
                    )));
                }
            }
        }
    }
    Ok(())
}

async fn grant_privileges(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    for statement in [
        format!("GRANT USAGE ON SCHEMA public TO {ROLE}"),
        format!("GRANT SELECT ON ALL TABLES IN SCHEMA public TO {ROLE}"),
    ] {
        sqlx::raw_sql(AssertSqlSafe(statement.clone()))
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::Error::new(e).context(format!("failed to run `{statement}`")))?;
    }
    let existing: Vec<String> = sqlx::query_scalar(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = 'public' AND tablename = ANY($1)",
    )
    .bind(PROTECTED_TABLES.as_slice())
    .fetch_all(&mut *tx)
    .await?;
    for table in &existing {
        // テーブル名は pg_tables から取った既知の名前 (PROTECTED_TABLES の要素) なのでそのまま埋め込む
        sqlx::raw_sql(AssertSqlSafe(format!(
            "REVOKE ALL ON TABLE public.\"{table}\" FROM {ROLE}"
        )))
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            anyhow::Error::new(e).context(format!("failed to revoke {table} from {ROLE}"))
        })?;
    }
    tx.commit().await?;
    tracing::debug!(role = ROLE, protected = ?existing, "SQL console privileges applied");
    Ok(())
}

/// 読み取り専用 SQL を実行する。`console` は [`console_pool`] で作った [`ROLE`] のプール。
/// `timeout` は 1 回の実行全体の上限 (通常は [`STATEMENT_TIMEOUT`]、テスト用に変えられる)
pub async fn execute(
    console: &PgPool,
    sql: &str,
    timeout: Duration,
) -> Result<SqlResult, SqlError> {
    let sql = sql.trim();
    if sql.is_empty() {
        return Err(SqlError::Rejected("SQL が空です".into()));
    }
    if sql.chars().count() > MAX_SQL_CHARS {
        return Err(SqlError::Rejected(format!(
            "SQL が長すぎます (上限 {MAX_SQL_CHARS} 文字)"
        )));
    }
    let statement = classify(sql)?;
    let sql = strip_trailing_trivia(sql);

    let mut conn = console.acquire().await.map_err(|error| {
        // ロールが無い / パスワードが違う等。設定の問題なので利用者に案内する
        SqlError::Unavailable(format!(
            "SQL コンソール用の DB 接続 (ロール {ROLE}) を開けません ({error})。README の手順でロールを用意してください"
        ))
    })?;
    let started = Instant::now();
    let deadline = started + timeout;
    let result = {
        let mut tx = conn.begin_with("BEGIN READ ONLY").await?;
        let result = run_in_transaction(&mut tx, sql, statement, deadline).await;
        // 読み取り専用なので常にロールバック (カーソルも SET LOCAL も一緒に消える)
        if let Err(error) = tx.rollback().await {
            tracing::warn!(error = %error, "failed to roll back the admin SQL transaction");
        }
        result
    };
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    release_session_state(conn).await;

    let (columns, rows, truncated) = result?;
    Ok(SqlResult {
        columns,
        row_count: rows.len(),
        rows,
        truncated,
        duration_ms,
    })
}

/// セッションに残りうる状態 (advisory lock) を片付けてから接続をプールに返す。片付けられなければ閉じる
async fn release_session_state(mut conn: PoolConnection<Postgres>) {
    match sqlx::query("SELECT pg_advisory_unlock_all()")
        .execute(&mut *conn)
        .await
    {
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(error = %error, "failed to reset the admin SQL connection; closing it");
            conn.close_on_drop();
        }
    }
}

type RowsOutput = (Vec<SqlColumn>, Vec<Vec<Option<String>>>, bool);

async fn run_in_transaction(
    conn: &mut PgConnection,
    sql: &str,
    statement: Statement<'_>,
    deadline: Instant,
) -> Result<RowsOutput, SqlError> {
    // (2) この接続が専用ロールのもので、保護テーブルを読めないことを毎回確かめる (fail closed)
    verify_console_role(conn).await?;

    // (5) 保護テーブルを読んでいないか、実行計画で確認する (実行はしない)。
    // EXPLAIN 文自体もプリペアドステートメントなので、複数文はここでも失敗する
    let explain_target = match statement {
        Statement::Select => Some(sql),
        Statement::Explain { inner } => Some(inner),
        Statement::Show => None,
    };
    if let Some(target) = explain_target {
        apply_deadline(conn, deadline).await?;
        let plan: serde_json::Value =
            sqlx::query_scalar(AssertSqlSafe(format!("EXPLAIN (FORMAT JSON) {target}")))
                .persistent(false)
                .fetch_one(&mut *conn)
                .await
                .map_err(SqlError::from_sqlx)?;
        let mut found = Vec::new();
        collect_protected_relations(&plan, &mut found);
        if !found.is_empty() {
            found.sort();
            found.dedup();
            return Err(SqlError::Rejected(format!(
                "秘密情報を含むテーブル ({}) は SQL コンソールから読めません",
                found.join(", ")
            )));
        }
    }

    // (6) 単一文であることの確認と、カラム情報の取得 (0 行でもカラム名を返せるように)。
    // `prepare` は他のクエリと同じく接続ごとのステートメントキャッシュ (LRU) に乗る。
    // 実行は次の DECLARE / FETCH で行うので、ここでは結果を取り出さない
    let prepared = conn
        .prepare(AssertSqlSafe(sql.to_owned()).into_sql_str())
        .await
        .map_err(SqlError::from_sqlx)?;
    let columns: Vec<SqlColumn> = prepared
        .columns()
        .iter()
        .map(|c| SqlColumn {
            name: c.name().to_owned(),
            type_name: c.type_info().name().to_owned(),
        })
        .collect();

    // (7) 実行。SELECT 系はセルを切り詰めるサブクエリに包んでカーソルにし、小分けに取り出す。
    // EXPLAIN / SHOW の出力は小さいのでそのまま (prepare が通っているので単一文)
    apply_deadline(conn, deadline).await?;
    let (rows, truncated) = match statement {
        Statement::Select => {
            sqlx::query(AssertSqlSafe(format!(
                "DECLARE admin_console_cursor NO SCROLL CURSOR FOR {}",
                wrap_for_cursor(sql, columns.len())
            )))
            .persistent(false)
            .execute(&mut *conn)
            .await
            .map_err(SqlError::from_sqlx)?;
            fetch_from_cursor(conn, deadline, columns.len()).await?
        }
        Statement::Explain { .. } | Statement::Show => {
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.to_owned()))
                .fetch_all(&mut *conn)
                .await
                .map_err(SqlError::from_sqlx)?;
            let mut out = Vec::new();
            let mut bytes = 0;
            let truncated = push_rows(&rows, &mut out, &mut bytes)?;
            (out, truncated)
        }
    };
    Ok((columns, rows, truncated))
}

/// 次のコマンドの `statement_timeout` を締切までの残り時間にする。残りが無ければタイムアウト扱い
async fn apply_deadline(conn: &mut PgConnection, deadline: Instant) -> Result<(), SqlError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err(SqlError::timed_out());
    }
    // 値は SET LOCAL に直接埋め込めないので文字列で組み立てる (数値なので安全)。最低 1ms
    let ms = remaining.as_millis().max(1);
    sqlx::raw_sql(AssertSqlSafe(format!("SET LOCAL statement_timeout = {ms}")))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// 接続のセッションユーザーが専用ロールで superuser でなく、保護テーブルを読めないことを確認する。
/// どれかが満たされなければ `Unavailable` (実行しない)
async fn verify_console_role(conn: &mut PgConnection) -> Result<(), SqlError> {
    let (session_user, superuser): (String, bool) = sqlx::query_as(
        "SELECT session_user::text, (SELECT rolsuper FROM pg_roles WHERE rolname = session_user)",
    )
    .fetch_one(&mut *conn)
    .await?;
    let readable: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT c.relname::text
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relname = ANY($1)
          AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND has_table_privilege(session_user, c.oid, 'SELECT')
        "#,
    )
    .bind(PROTECTED_TABLES.as_slice())
    .fetch_all(&mut *conn)
    .await?;
    if session_user != ROLE || superuser || !readable.is_empty() {
        return Err(SqlError::Unavailable(format!(
            "SQL コンソール用の DB 接続の権限が正しくありません (session_user: {session_user}, superuser: {superuser}, 読める保護テーブル: {readable:?})。README の手順でロール {ROLE} を直してください"
        )));
    }
    Ok(())
}

/// SELECT 系の文を、各列を text にして `MAX_CELL_CHARS` + 1 文字に切り詰めるサブクエリに包む。
/// 列は位置で別名 (`q(c1, c2, ...)`) を付けるので、同名の列があっても曖昧にならない。
/// 末尾が `-- コメント` でも壊れないよう改行で囲む。列が無い文 (`SELECT FROM t`) はそのまま
fn wrap_for_cursor(sql: &str, column_count: usize) -> String {
    if column_count == 0 {
        return sql.to_owned();
    }
    let aliases: Vec<String> = (1..=column_count).map(|i| format!("c{i}")).collect();
    let projections: Vec<String> = aliases
        .iter()
        .map(|a| format!("left(q.{a}::text, {})", MAX_CELL_CHARS + 1))
        .collect();
    format!(
        "SELECT {} FROM (\n{sql}\n) AS q({})",
        projections.join(", "),
        aliases.join(", ")
    )
}

/// 1 回の FETCH で取り出す行数。セルは最大 `MAX_CELL_CHARS + 1` 文字 (UTF-8 で最大 4 バイト/文字) なので、
/// 行数 × 列数 × セル上限が `MAX_FETCH_BATCH_BYTES` を超えないようにする (列が 1,600 あっても 1 行ずつになるだけ)
fn fetch_batch_rows(column_count: usize) -> usize {
    let cell_max_bytes = (MAX_CELL_CHARS + 1) * 4;
    let row_max_bytes = column_count.max(1) * cell_max_bytes;
    (MAX_FETCH_BATCH_BYTES / row_max_bytes).clamp(1, MAX_FETCH_BATCH)
}

/// カーソルから小分けに取り出し、行数かバイト数の上限で打ち切る。FETCH ごとに締切までの残り時間を設定し直す。
/// 戻り値は (行, truncated)
async fn fetch_from_cursor(
    conn: &mut PgConnection,
    deadline: Instant,
    column_count: usize,
) -> Result<(Vec<Vec<Option<String>>>, bool), SqlError> {
    let batch_rows = fetch_batch_rows(column_count);
    let mut out = Vec::new();
    let mut bytes = 0usize;
    loop {
        apply_deadline(conn, deadline).await?;
        let batch = sqlx::raw_sql(AssertSqlSafe(format!(
            "FETCH FORWARD {batch_rows} FROM admin_console_cursor"
        )))
        .fetch_all(&mut *conn)
        .await
        .map_err(SqlError::from_sqlx)?;
        let exhausted = batch.len() < batch_rows;
        if push_rows(&batch, &mut out, &mut bytes)? {
            return Ok((out, true));
        }
        if exhausted {
            return Ok((out, false));
        }
    }
}

/// 取り出した行を `out` に移す。`MAX_ROWS` 行に達したか、その行を足すと `MAX_RESULT_BYTES` バイトを超えるなら
/// その行は含めずに true (打ち切り) を返す (返す量が上限を超えない)
fn push_rows(
    rows: &[sqlx::postgres::PgRow],
    out: &mut Vec<Vec<Option<String>>>,
    bytes: &mut usize,
) -> Result<bool, SqlError> {
    for row in rows {
        if out.len() >= MAX_ROWS {
            return Ok(true);
        }
        let mut row_bytes = 0usize;
        let mut values = Vec::with_capacity(row.len());
        for i in 0..row.len() {
            let raw = row.try_get_raw(i)?;
            if raw.is_null() {
                values.push(None);
                continue;
            }
            let text = raw.as_str().map_err(|e| sqlx::Error::ColumnDecode {
                index: i.to_string(),
                source: e,
            })?;
            // サーバー側で MAX_CELL_CHARS + 1 文字に切ってあるので、超えていれば印を付けて MAX_CELL_CHARS に揃える
            let value = match text.char_indices().nth(MAX_CELL_CHARS) {
                Some((end, _)) => format!("{}{CELL_TRUNCATED_MARK}", &text[..end]),
                None => text.to_owned(),
            };
            row_bytes += value.len();
            values.push(Some(value));
        }
        if *bytes + row_bytes > MAX_RESULT_BYTES {
            return Ok(true);
        }
        *bytes += row_bytes;
        out.push(values);
    }
    Ok(false)
}

/// 監査ログ・履歴に残す用に、SQL の文字列リテラル (`'...'`、`E'...'`、`$tag$...$tag$`) の中身を `…` に、
/// コメント (`-- ...`、`/* ... */`) を空白に置き換える。識別子 (`"..."`) と数値はそのまま。
/// 管理者が貼り付けたトークンや cookie が履歴として残らないようにするためで、構文として正しい必要はない
pub fn redact_sql(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut rest = sql;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("--") {
            // 行コメント: 改行まで捨てる (改行は残す)
            rest = after.find('\n').map_or("", |i| &after[i..]);
        } else if let Some(after) = rest.strip_prefix("/*") {
            // ブロックコメント (入れ子可)
            let mut depth = 1usize;
            let mut r = after;
            while depth > 0 {
                match (r.find("/*"), r.find("*/")) {
                    (Some(o), Some(c)) if o < c => {
                        depth += 1;
                        r = &r[o + 2..];
                    }
                    (_, Some(c)) => {
                        depth -= 1;
                        r = &r[c + 2..];
                    }
                    _ => {
                        r = "";
                        break;
                    }
                }
            }
            out.push(' ');
            rest = r;
        } else if let Some(after) = rest.strip_prefix('\'') {
            // 標準の文字列 ('' は引用符のエスケープ)。直前が E / e なら \ エスケープも読み飛ばす
            let escapes = out.ends_with(['E', 'e']);
            out.push_str("'…'");
            rest = skip_quoted(after, '\'', escapes);
        } else if let Some(after) = rest.strip_prefix('"') {
            // 識別子はそのまま ("" は引用符のエスケープ)
            let end = skip_quoted(after, '"', false);
            out.push('"');
            out.push_str(&after[..after.len() - end.len()]);
            rest = end;
        } else if rest.starts_with('$')
            && let Some((tag, after)) = dollar_tag(rest)
        {
            // ドル引用: 同じタグまで捨てる
            out.push_str(tag);
            out.push('…');
            out.push_str(tag);
            rest = match after.find(tag) {
                Some(i) => &after[i + tag.len()..],
                None => "",
            };
        } else {
            let ch = rest.chars().next().expect("non-empty");
            out.push(ch);
            rest = &rest[ch.len_utf8()..];
        }
    }
    out
}

/// `quote` で閉じられるまで読み飛ばし、閉じ引用符の後ろを返す (閉じ引用符も含めて消費する)。
/// `quote` の 2 連続はエスケープ。`backslash_escapes` なら `\x` も読み飛ばす。閉じていなければ空文字列
fn skip_quoted(s: &str, quote: char, backslash_escapes: bool) -> &str {
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        if backslash_escapes && c == '\\' {
            chars.next();
        } else if c == quote {
            let after = &s[i + c.len_utf8()..];
            if after.starts_with(quote) {
                chars.next();
            } else {
                return after;
            }
        }
    }
    ""
}

/// 先頭の `$tag$` (タグは空か識別子) を返す。`$1` のようなプレースホルダは対象外
fn dollar_tag(s: &str) -> Option<(&str, &str)> {
    let after = s.strip_prefix('$')?;
    let end = after
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    let tag = &after[..end];
    if !after[end..].starts_with('$') || tag.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let full = &s[..end + 2];
    Some((full, &s[end + 2..]))
}

/// 末尾の `;` と空白、末尾行の `-- コメント` を取り除く (サブクエリに包めるように)
fn strip_trailing_trivia(sql: &str) -> &str {
    let mut s = sql.trim_end();
    loop {
        if let Some(rest) = s.strip_suffix(';') {
            s = rest.trim_end();
            continue;
        }
        let last_line_start = s.rfind('\n').map_or(0, |i| i + 1);
        if s[last_line_start..].trim_start().starts_with("--") && last_line_start > 0 {
            s = s[..last_line_start].trim_end();
            continue;
        }
        return s;
    }
}

/// 実行計画 (EXPLAIN FORMAT JSON) の中から保護テーブルの `Relation Name` を集める。
/// ノードの入れ子 (`Plans`)、InitPlan / SubPlan、CTE もすべて同じ JSON の中に現れるので、全体を再帰的に見る
fn collect_protected_relations(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(name)) = map.get("Relation Name")
                && REJECTED_RELATIONS.contains(&name.as_str())
            {
                found.push(name.clone());
            }
            for child in map.values() {
                collect_protected_relations(child, found);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_protected_relations(item, found);
            }
        }
        _ => {}
    }
}

/// 先頭のキーワードで文の種類を決める。読み取り専用の文以外は拒否する
fn classify(sql: &str) -> Result<Statement<'_>, SqlError> {
    let mut rest = skip_trivia(sql);
    // `(SELECT ...) UNION (SELECT ...)` のように括弧で始まる SELECT も通す
    while let Some(after) = rest.strip_prefix('(') {
        rest = skip_trivia(after);
    }
    let (keyword, after_keyword) = take_word(rest);
    match keyword.to_ascii_lowercase().as_str() {
        "select" | "with" | "values" | "table" => Ok(Statement::Select),
        "show" => Ok(Statement::Show),
        "explain" => Ok(Statement::Explain {
            inner: explain_target(after_keyword),
        }),
        _ => Err(SqlError::Rejected(
            "読み取り専用の文 (SELECT / WITH / VALUES / TABLE / EXPLAIN / SHOW) だけ実行できます"
                .into(),
        )),
    }
}

/// `EXPLAIN` の後ろからオプション (`(ANALYZE, FORMAT JSON)` か旧式の `ANALYZE` / `VERBOSE`) を読み飛ばし、
/// 対象の文を返す。オプションの形が想定外なら残りをそのまま返し、その後の `EXPLAIN (FORMAT JSON)` が失敗して
/// 拒否される (安全側に倒れる)
fn explain_target(after_explain: &str) -> &str {
    let mut rest = skip_trivia(after_explain);
    if let Some(opts) = rest.strip_prefix('(') {
        // オプションは括弧の入れ子も引用符も持たないので、最初の `)` まで
        return match opts.find(')') {
            Some(end) => skip_trivia(&opts[end + 1..]),
            None => rest,
        };
    }
    loop {
        let (word, after) = take_word(rest);
        if matches!(
            word.to_ascii_lowercase().as_str(),
            "analyze" | "analyse" | "verbose"
        ) {
            rest = skip_trivia(after);
        } else {
            return rest;
        }
    }
}

/// 先頭の空白と、`-- ...` / `/* ... */` (入れ子可) のコメントを読み飛ばす
fn skip_trivia(mut s: &str) -> &str {
    loop {
        s = s.trim_start();
        if let Some(after) = s.strip_prefix("--") {
            s = after.find('\n').map_or("", |i| &after[i + 1..]);
        } else if let Some(after) = s.strip_prefix("/*") {
            let mut depth = 1usize;
            let mut rest = after;
            loop {
                let next_open = rest.find("/*");
                let next_close = rest.find("*/");
                match (next_open, next_close) {
                    (Some(o), Some(c)) if o < c => {
                        depth += 1;
                        rest = &rest[o + 2..];
                    }
                    (_, Some(c)) => {
                        depth -= 1;
                        rest = &rest[c + 2..];
                        if depth == 0 {
                            break;
                        }
                    }
                    // 閉じていないコメント。残りは全部コメント扱い (Postgres も構文エラーにする)
                    _ => {
                        rest = "";
                        break;
                    }
                }
            }
            s = rest;
        } else {
            return s;
        }
    }
}

/// 先頭の単語 (英字・数字・`_`) とその残りを返す
fn take_word(s: &str) -> (&str, &str) {
    let end = s
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(s.len());
    (&s[..end], &s[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(sql: &str) -> Result<Statement<'_>, String> {
        classify(sql).map_err(|e| e.to_string())
    }

    #[test]
    fn accepts_read_only_statements() {
        assert_eq!(kind("SELECT 1").unwrap(), Statement::Select);
        assert_eq!(
            kind("  with x as (select 1) select * from x").unwrap(),
            Statement::Select
        );
        assert_eq!(kind("VALUES (1), (2)").unwrap(), Statement::Select);
        assert_eq!(kind("TABLE guilds").unwrap(), Statement::Select);
        assert_eq!(
            kind("(SELECT 1) UNION (SELECT 2)").unwrap(),
            Statement::Select
        );
        assert_eq!(kind("SHOW server_version").unwrap(), Statement::Show);
        assert_eq!(
            kind("-- comment\n/* block /* nested */ */ select 1").unwrap(),
            Statement::Select
        );
    }

    #[test]
    fn rejects_anything_else() {
        for sql in [
            "INSERT INTO events DEFAULT VALUES",
            "UPDATE events SET name = 'x'",
            "DELETE FROM events",
            "DROP TABLE events",
            "SET statement_timeout = 0",
            "RESET statement_timeout",
            "DO $$ BEGIN END $$",
            "COPY events TO STDOUT",
            "BEGIN",
            "COMMIT",
            "LOCK TABLE events",
            "DECLARE c CURSOR FOR SELECT 1",
            "FETCH ALL FROM c",
            "",
            "/* only a comment */",
            "-- unterminated",
        ] {
            assert!(kind(sql).is_err(), "{sql:?} should be rejected");
        }
    }

    #[test]
    fn extracts_explain_target() {
        assert_eq!(
            kind("EXPLAIN SELECT 1").unwrap(),
            Statement::Explain { inner: "SELECT 1" }
        );
        assert_eq!(
            kind("explain analyze verbose select 1").unwrap(),
            Statement::Explain { inner: "select 1" }
        );
        assert_eq!(
            kind("EXPLAIN (ANALYZE, FORMAT JSON) SELECT 1").unwrap(),
            Statement::Explain { inner: "SELECT 1" }
        );
        assert_eq!(
            kind("EXPLAIN (ANALYZE) -- c\n SELECT 1").unwrap(),
            Statement::Explain { inner: "SELECT 1" }
        );
    }

    #[test]
    fn redacts_literals_and_comments_but_keeps_structure() {
        assert_eq!(
            redact_sql(
                "SELECT 'secret', \"col\", 42 FROM t WHERE a = 'x''y' -- tok\nAND b = E'a\\'b'"
            ),
            "SELECT '…', \"col\", 42 FROM t WHERE a = '…' \nAND b = E'…'"
        );
        assert_eq!(
            redact_sql("SELECT $$raw 'token'$$, $tag$x$tag$, $1 /* c /* n */ */ + 1"),
            "SELECT $$…$$, $tag$…$tag$, $1   + 1"
        );
        // 閉じていないリテラルやコメントは末尾まで伏せる
        assert_eq!(redact_sql("SELECT 'open"), "SELECT '…'");
        assert_eq!(redact_sql("SELECT 1 /* open"), "SELECT 1  ");
        assert_eq!(redact_sql("SELECT \"a\"\"b\""), "SELECT \"a\"\"b\"");
    }

    #[test]
    fn fetch_batch_shrinks_with_wide_rows() {
        assert_eq!(fetch_batch_rows(1), MAX_FETCH_BATCH);
        assert_eq!(fetch_batch_rows(5), MAX_FETCH_BATCH);
        assert!(fetch_batch_rows(50) < MAX_FETCH_BATCH);
        assert_eq!(fetch_batch_rows(1600), 1);
    }

    #[test]
    fn finds_protected_relations_anywhere_in_the_plan() {
        let plan = serde_json::json!([{
            "Plan": {
                "Node Type": "Aggregate",
                "Plans": [
                    { "Node Type": "Seq Scan", "Relation Name": "events" },
                    {
                        "Node Type": "Nested Loop",
                        "Plans": [{ "Node Type": "Index Scan", "Relation Name": "session", "Schema": "public" }]
                    }
                ]
            }
        }]);
        let mut found = Vec::new();
        collect_protected_relations(&plan, &mut found);
        assert_eq!(found, vec!["session"]);
    }
}
