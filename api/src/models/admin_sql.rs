//! 管理コンソールの読み取り専用 SQL コンソール (#36)。
//!
//! 任意の SQL を受け取るので、壊さない・漏らさないための層を重ねる:
//!
//! 1. 先頭のキーワードを `SELECT` / `WITH` / `VALUES` / `TABLE` / `EXPLAIN` / `SHOW` に限る
//!    (`SET` / `DO` / `COPY` / DDL / `BEGIN` などはここで弾く)
//! 2. `BEGIN READ ONLY` のトランザクションで実行する (`WITH ... DELETE` のような書き込みは Postgres が拒否する)
//!    ので、稼働中の旧 Bot と共有している `events` などを誤って壊せない
//! 3. `SET LOCAL statement_timeout` で長いクエリを打ち切る
//! 4. **専用の DB ロールで実行する (P0)**: `SET LOCAL ROLE` で [`ROLE`] (`discalendar_sql_console`、NOLOGIN、
//!    superuser ではない) に切り替える。このロールには `public` スキーマのテーブルの SELECT だけを与え、
//!    Better Auth の `account` (Discord の access / refresh token)、`session` (セッショントークン)、`verification`
//!    ([`PROTECTED_TABLES`]) は権限を外してあるので、`table_to_xml('account', ...)` や `query_to_xml(...)` のように
//!    関数の内部で実行される SQL でも読めない (`permission denied`)。superuser でないので `pg_read_file` や
//!    他の接続の `pg_terminate_backend` も使えず、プランナ統計 `pg_statistic` (列のサンプル値に実トークンが入る) も
//!    読めない (`pg_stats` ビューは権限のある表の行しか見せない)。ロールの作成と権限付与は api の起動時に
//!    [`setup_role`] が行い、できない環境では手で作る (README)。実行のたびに保護テーブルを読めないことを
//!    `has_table_privilege` で確かめ、ロールが使えなければ実行しない (fail closed)
//! 5. **実行計画での事前判定**: `EXPLAIN (FORMAT JSON)` の計画を走査し、`Relation Name` に保護テーブルが出てくる文は
//!    実行前に拒否する (4 の多層防御。権限エラーより分かりやすいメッセージにもなる)。SQL の文字列を見て判定すると
//!    別名・サブクエリ・`U&"..."` の識別子などですり抜けられるが、計画に出てくる名前はどう書いても変わらない
//! 6. 文字列はプリペアドステートメントとして `prepare` する (カラム名・型の取得も兼ねる)。Postgres は
//!    1 つのプリペアドステートメントに複数の文を入れられないので、`SELECT 1; SET ...` のような複数文は
//!    ここ (と 5 の EXPLAIN) で失敗する
//! 7. 行数とサイズ: SELECT 系は `SELECT left(c1::text, N), ... FROM (<sql>) AS q(c1, ...)` に包んで
//!    `DECLARE ... CURSOR` にし、`FETCH` を小分けにして [`MAX_ROWS`] 行・[`MAX_RESULT_BYTES`] バイトで打ち切る。
//!    セルの切り詰め ([`MAX_CELL_CHARS`]) もサーバー側で行うので、巨大な値でサーバーから送られてくる量が膨らまない
//! 8. 実行後は `pg_advisory_unlock_all()` で、セッションに残りうる advisory lock
//!    (`pg_advisory_lock(...)` はロールバックでは解放されない) を外してから接続をプールに返す。外せなければ接続を閉じる
//!
//! 結果の値は text にキャストして simple query protocol (text format) で受け取り、型ごとのデコードをせずに
//! 文字列表現のまま返す (psql とほぼ同じ。boolean は `true` / `false`。NULL は null)。

use std::time::{Duration, Instant};

use serde::Serialize;
use sqlx::{
    AssertSqlSafe, Column as _, Connection as _, Executor as _, PgConnection, PgPool, Row as _,
    SqlSafeStr as _, Statement as _, TypeInfo as _, ValueRef as _, pool::PoolConnection,
    postgres::Postgres,
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
/// カーソルから 1 回に取り出す行数
const FETCH_BATCH: usize = 100;
/// 1 文の実行時間の上限 (`statement_timeout`)
pub const STATEMENT_TIMEOUT: Duration = Duration::from_secs(10);
/// 受け付ける SQL の長さ (文字数) の上限。監査ログにもそのまま残す
pub const MAX_SQL_CHARS: usize = 10_000;
/// SQL コンソールの実行に使う DB ロール (NOLOGIN、superuser ではない。[`setup_role`] が作る)
pub const ROLE: &str = "discalendar_sql_console";
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
    /// SQL コンソール用のロールが使えない (作られていない / 権限が正しくない)。実行していない
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

/// SQL コンソール用のロール [`ROLE`] を用意する (api の起動時に呼ぶ。冪等)。
///
/// - ロールが無ければ `CREATE ROLE ... NOLOGIN` し、api の接続ユーザーをメンバーにする (`SET ROLE` に必要)
/// - `public` スキーマの全テーブルに SELECT を与え、[`PROTECTED_TABLES`] (存在するもの) からは取り上げる。
///   Better Auth のテーブルは web が起動時に作るので、api を先に起動した環境では次回の起動で付与される
///   (それまでロールは読めない = 安全側)
///
/// `CREATE ROLE` には CREATEROLE か superuser が要る。compose の Postgres (`POSTGRES_USER`) は superuser なので
/// そのまま通る。権限が無い環境では Err を返すので、README の SQL を手で流す (コンソールは [`execute`] が
/// 毎回ロールを検証して、使えなければ実行しない)
pub async fn setup_role(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    let role_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = $1)")
            .bind(ROLE)
            .fetch_one(&mut *tx)
            .await?;
    if !role_exists {
        // 同時に起動した別インスタンスと競合したら duplicate_object (42710) か pg_authid の一意制約違反 (23505)
        // になるので、それは成功扱い
        if let Err(error) = sqlx::raw_sql(AssertSqlSafe(format!("CREATE ROLE {ROLE} NOLOGIN")))
            .execute(&mut *tx)
            .await
        {
            let duplicate = matches!(&error, sqlx::Error::Database(db)
                if matches!(db.code().as_deref(), Some("42710" | "23505")));
            if !duplicate {
                return Err(anyhow::Error::new(error).context(format!(
                    "failed to create the role {ROLE} (CREATEROLE or superuser is required)"
                )));
            }
            // 競合相手のトランザクションが進んでいるので、こちらはやり直す
            tx.rollback().await?;
            tx = pool.begin().await?;
        }
    }
    for statement in [
        format!("GRANT {ROLE} TO CURRENT_USER"),
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
    tracing::info!(role = ROLE, protected = ?existing, "SQL console role is ready");
    Ok(())
}

/// 読み取り専用 SQL を実行する。`timeout` は `statement_timeout` (通常は [`STATEMENT_TIMEOUT`]、テスト用に変えられる)
pub async fn execute(pool: &PgPool, sql: &str, timeout: Duration) -> Result<SqlResult, SqlError> {
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

    let mut conn = pool.acquire().await?;
    let started = Instant::now();
    let result = {
        let mut tx = conn.begin_with("BEGIN READ ONLY").await?;
        let result = run_in_transaction(&mut tx, sql, statement, timeout).await;
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
    timeout: Duration,
) -> Result<RowsOutput, SqlError> {
    // (3) 値は SET LOCAL に直接埋め込めないので文字列で組み立てる (数値なので安全)
    let timeout_ms = timeout.as_millis().max(1);
    sqlx::raw_sql(AssertSqlSafe(format!(
        "SET LOCAL statement_timeout = {timeout_ms}"
    )))
    .execute(&mut *conn)
    .await?;

    // (5) 保護テーブルを読んでいないか、実行計画で確認する (実行はしない。api のロールで計画だけ立てるので、
    // 権限エラーではなく分かりやすいメッセージで拒否できる)。EXPLAIN 文自体もプリペアドステートメントなので、
    // 複数文はここでも失敗する
    let explain_target = match statement {
        Statement::Select => Some(sql),
        Statement::Explain { inner } => Some(inner),
        Statement::Show => None,
    };
    if let Some(target) = explain_target {
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

    // (4) 専用ロールに切り替え、保護テーブルを読めないことを毎回確かめる (fail closed)。
    // ここから先 (prepare / DECLARE / FETCH) はすべてこのロールの権限で動く
    switch_to_console_role(conn).await?;

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
    let rows = match statement {
        Statement::Select => {
            sqlx::query(AssertSqlSafe(format!(
                "DECLARE admin_console_cursor NO SCROLL CURSOR FOR {}",
                wrap_for_cursor(sql, columns.len())
            )))
            .persistent(false)
            .execute(&mut *conn)
            .await
            .map_err(SqlError::from_sqlx)?;
            fetch_from_cursor(conn).await?
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
    Ok((columns, rows.0, rows.1))
}

/// `SET LOCAL ROLE` で専用ロールに切り替え、そのロールが保護テーブルを読めないことを確認する。
/// ロールが無い・メンバーでない・権限が残っている、のどれでも `Unavailable` (実行しない)
async fn switch_to_console_role(conn: &mut PgConnection) -> Result<(), SqlError> {
    sqlx::raw_sql(AssertSqlSafe(format!("SET LOCAL ROLE {ROLE}")))
        .execute(&mut *conn)
        .await
        .map_err(|error| match error {
            sqlx::Error::Database(db) => SqlError::Unavailable(format!(
                "SQL コンソール用の DB ロール {ROLE} に切り替えられません ({})。README の手順でロールを作成してください",
                db.message()
            )),
            other => SqlError::Other(other),
        })?;
    let readable: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT c.relname::text
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relname = ANY($1)
          AND c.relkind IN ('r', 'p', 'v', 'm', 'f')
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND has_table_privilege(current_user, c.oid, 'SELECT')
        "#,
    )
    .bind(PROTECTED_TABLES.as_slice())
    .fetch_all(&mut *conn)
    .await?;
    let superuser: bool =
        sqlx::query_scalar("SELECT rolsuper FROM pg_roles WHERE rolname = current_user")
            .fetch_one(&mut *conn)
            .await?;
    if superuser || !readable.is_empty() {
        return Err(SqlError::Unavailable(format!(
            "SQL コンソール用の DB ロール {ROLE} の権限が正しくありません (superuser: {superuser}, 読める保護テーブル: {readable:?})。README の手順で権限を直してください"
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

/// カーソルから小分けに取り出し、行数かバイト数の上限で打ち切る。戻り値は (行, truncated)
async fn fetch_from_cursor(
    conn: &mut PgConnection,
) -> Result<(Vec<Vec<Option<String>>>, bool), SqlError> {
    let mut out = Vec::new();
    let mut bytes = 0usize;
    loop {
        let batch = sqlx::raw_sql(AssertSqlSafe(format!(
            "FETCH FORWARD {FETCH_BATCH} FROM admin_console_cursor"
        )))
        .fetch_all(&mut *conn)
        .await
        .map_err(SqlError::from_sqlx)?;
        let exhausted = batch.len() < FETCH_BATCH;
        if push_rows(&batch, &mut out, &mut bytes)? {
            return Ok((out, true));
        }
        if exhausted {
            return Ok((out, false));
        }
    }
}

/// 取り出した行を `out` に移す。`MAX_ROWS` 行か `MAX_RESULT_BYTES` バイトを超えたら true (打ち切り) を返す
fn push_rows(
    rows: &[sqlx::postgres::PgRow],
    out: &mut Vec<Vec<Option<String>>>,
    bytes: &mut usize,
) -> Result<bool, SqlError> {
    for row in rows {
        if out.len() >= MAX_ROWS || *bytes >= MAX_RESULT_BYTES {
            return Ok(true);
        }
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
            *bytes += value.len();
            values.push(Some(value));
        }
        out.push(values);
    }
    Ok(false)
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
