//! 管理コンソールの読み取り専用 SQL コンソール (#36)。
//!
//! 任意の SQL を受け取るので、壊さない・漏らさないための層を重ねる:
//!
//! 1. 先頭のキーワードを `SELECT` / `WITH` / `VALUES` / `TABLE` / `EXPLAIN` / `SHOW` に限る
//!    (`SET` / `DO` / `COPY` / DDL / `BEGIN` などはここで弾く)
//! 2. `BEGIN READ ONLY` のトランザクションで実行する (`WITH ... DELETE` のような書き込みは Postgres が拒否する)
//!    ので、稼働中の旧 Bot と共有している `events` などを誤って壊せない
//! 3. `SET LOCAL statement_timeout` で長いクエリを打ち切る
//! 4. **秘密情報の保護 (P0)**: Better Auth の `account` (Discord の access / refresh token)、`session` (セッショントークン)、
//!    `verification` を読む文は `EXPLAIN (FORMAT JSON)` の実行計画を走査して、実行前に拒否する。
//!    SQL の文字列を見て判定すると別名・サブクエリ・`U&"..."` の識別子などですり抜けられるが、
//!    計画に出てくる `Relation Name` はどう書いても変わらない。実行前に判定するので、
//!    型キャストの失敗などのエラーメッセージ経由で値が漏れることもない。
//!    プランナ統計 (`pg_statistic` とそのビュー `pg_stats`) には列のサンプル値 (`histogram_bounds` /
//!    `most_common_vals`) としてトークンの実値が入るので、これらも同様に拒否する
//! 5. 文字列はプリペアドステートメントとして `prepare` する (カラム名・型の取得も兼ねる)。Postgres は
//!    1 つのプリペアドステートメントに複数の文を入れられないので、`SELECT 1; SET ...` のような複数文は
//!    ここ (と 4 の EXPLAIN) で失敗する
//! 6. 行数は `DECLARE ... CURSOR` + `FETCH` で [`MAX_ROWS`] + 1 件までしか取り出さない
//!    (大きな表を `SELECT *` してもサーバーが全件を送ってこない)
//!
//! 結果の値は simple query protocol (text format) で受け取り、型ごとのデコードをせずに
//! psql と同じ文字列表現で返す (NULL は null)。
//!
//! 想定する脅威は「管理者がうっかりトークンを表示・共有してしまう」こと。ホワイトリストの管理者が
//! 意図的に迂回すること (DB ロールが superuser なら `pg_read_binary_file` でデータファイルを読む等) は
//! 対象外で、そこは DB 側の権限 (api のロールを superuser にしない) で守る。

use std::time::{Duration, Instant};

use serde::Serialize;
use sqlx::{
    AssertSqlSafe, Column as _, Executor as _, PgConnection, PgPool, Row as _, SqlSafeStr as _,
    Statement as _, TypeInfo as _, ValueRef as _,
};
use utoipa::ToSchema;

/// 1 回の実行で返す最大行数。これを超えた分は捨てて `truncated` を立てる
pub const MAX_ROWS: usize = 500;
/// 1 文の実行時間の上限 (`statement_timeout`)
pub const STATEMENT_TIMEOUT: Duration = Duration::from_secs(10);
/// 受け付ける SQL の長さ (文字数) の上限。監査ログにもそのまま残す
pub const MAX_SQL_CHARS: usize = 10_000;
/// 読み取りを禁止するテーブル。スキーマに関係なく名前で判定する。
/// Better Auth のトークン類を持つ 3 テーブルと、その列のサンプル値を持つプランナ統計
/// (`pg_stats` / `pg_stats_ext*` はビューなので計画上は `pg_statistic*` として現れるが、念のため名前も入れる)
pub const PROTECTED_TABLES: [&str; 8] = [
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
    /// 行ごとの値 (カラム順)。値は Postgres のテキスト表現、NULL は null
    #[schema(value_type = Vec<Vec<Option<String>>>)]
    pub rows: Vec<Vec<Option<String>>>,
    /// 返した行数 (`rows.len()`)
    pub row_count: usize,
    /// `MAX_ROWS` を超えたので打ち切ったか
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
    /// Postgres がエラーを返した (構文エラー、読み取り専用違反、タイムアウトなど)。メッセージはそのまま見せてよい
    #[error("{0}")]
    Query(String),
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
    /// SELECT / WITH / VALUES / TABLE。カーソルで行数を絞って実行する
    Select,
    /// EXPLAIN。`inner` は EXPLAIN の対象の文 (保護テーブルの判定にはこちらを使う)
    Explain { inner: &'a str },
    /// SHOW。対象のテーブルが無いので計画の確認は不要
    Show,
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

    let mut tx = pool.begin_with("BEGIN READ ONLY").await?;
    let started = Instant::now();
    let result = run_in_transaction(&mut tx, sql, statement, timeout).await;
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    // 読み取り専用なので常にロールバック (カーソルも一緒に閉じる)。失敗しても結果には影響しない
    if let Err(error) = tx.rollback().await {
        tracing::warn!(error = %error, "failed to roll back the admin SQL transaction");
    }
    let (columns, rows, truncated) = result?;
    Ok(SqlResult {
        columns,
        row_count: rows.len(),
        rows,
        truncated,
        duration_ms,
    })
}

type RowsOutput = (Vec<SqlColumn>, Vec<Vec<Option<String>>>, bool);

async fn run_in_transaction(
    conn: &mut PgConnection,
    sql: &str,
    statement: Statement<'_>,
    timeout: Duration,
) -> Result<RowsOutput, SqlError> {
    // 値は SET LOCAL に直接埋め込めないので文字列で組み立てる (数値なので安全)
    let timeout_ms = timeout.as_millis().max(1);
    sqlx::raw_sql(AssertSqlSafe(format!(
        "SET LOCAL statement_timeout = {timeout_ms}"
    )))
    .execute(&mut *conn)
    .await?;

    // (4) 保護テーブルを読んでいないか、実行計画で確認する (実行はしない)。
    // EXPLAIN 文自体もプリペアドステートメントなので、複数文はここでも失敗する
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

    // (5) 単一文であることの確認と、カラム情報の取得 (0 行でもカラム名を返せるように)。
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

    // (6) 実行。SELECT 系はカーソルで MAX_ROWS + 1 件だけ取り出す。EXPLAIN / SHOW の出力は小さいのでそのまま
    let rows = match statement {
        Statement::Select => {
            sqlx::query(AssertSqlSafe(format!(
                "DECLARE admin_console_cursor NO SCROLL CURSOR FOR {sql}"
            )))
            .persistent(false)
            .execute(&mut *conn)
            .await
            .map_err(SqlError::from_sqlx)?;
            sqlx::raw_sql(AssertSqlSafe(format!(
                "FETCH FORWARD {} FROM admin_console_cursor",
                MAX_ROWS + 1
            )))
            .fetch_all(&mut *conn)
            .await
            .map_err(SqlError::from_sqlx)?
        }
        Statement::Explain { .. } | Statement::Show => {
            // prepare が通っているので単一文。simple protocol で実行してテキスト表現で受け取る
            sqlx::raw_sql(AssertSqlSafe(sql.to_owned()))
                .fetch_all(&mut *conn)
                .await
                .map_err(SqlError::from_sqlx)?
        }
    };

    let truncated = rows.len() > MAX_ROWS;
    let mut out = Vec::with_capacity(rows.len().min(MAX_ROWS));
    for row in rows.iter().take(MAX_ROWS) {
        let mut values = Vec::with_capacity(columns.len());
        for i in 0..row.len() {
            let raw = row.try_get_raw(i)?;
            if raw.is_null() {
                values.push(None);
            } else {
                let text = raw
                    .as_str()
                    .map_err(|e| sqlx::Error::ColumnDecode {
                        index: i.to_string(),
                        source: e,
                    })?
                    .to_owned();
                values.push(Some(text));
            }
        }
        out.push(values);
    }
    Ok((columns, out, truncated))
}

/// 実行計画 (EXPLAIN FORMAT JSON) の中から保護テーブルの `Relation Name` を集める。
/// ノードの入れ子 (`Plans`)、InitPlan / SubPlan、CTE もすべて同じ JSON の中に現れるので、全体を再帰的に見る
fn collect_protected_relations(value: &serde_json::Value, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(name)) = map.get("Relation Name")
                && PROTECTED_TABLES.contains(&name.as_str())
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
