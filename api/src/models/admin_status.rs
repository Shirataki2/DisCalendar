//! 管理コンソールの稼働状況 (`GET /admin/status`、#37) のうち DB に聞く部分。
//!
//! `_sqlx_migrations` の内容と、実行ファイルに埋め込まれたマイグレーション
//! (`sqlx::migrate!("./migrations")`) を突き合わせて、未適用・失敗・チェックサム不一致を出す。
//! チェックサムは旧版と共有していて既存ファイルを変更してはいけない (AGENTS.md の P0) ので、
//! 不一致が出ていることに運用中に気付けるようにしておく。

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

/// `_sqlx_migrations` の 1 行
#[derive(Debug, Serialize, ToSchema)]
pub struct AppliedMigration {
    #[schema(example = 20_260_823_170_047_i64)]
    pub version: i64,
    #[schema(example = "create admin audit logs")]
    pub description: String,
    pub installed_on: DateTime<Utc>,
    /// 適用が完走したか。false のまま残っている行があれば DB は中途半端な状態
    pub success: bool,
    pub execution_time_ms: i64,
}

/// 実行ファイルに埋め込まれているが `_sqlx_migrations` に無いマイグレーション
#[derive(Debug, Serialize, ToSchema)]
pub struct PendingMigration {
    pub version: i64,
    pub description: String,
}

/// マイグレーションの適用状況
#[derive(Debug, Serialize, ToSchema)]
pub struct MigrationStatus {
    /// `_sqlx_migrations` の行数
    pub applied_count: i64,
    /// 適用済みのうち最新のもの (1 件も無ければ null)
    pub latest: Option<AppliedMigration>,
    /// まだ適用されていないもの (api の起動時に適用されるので、通常は空)
    pub pending: Vec<PendingMigration>,
    /// `success = false` のまま残っている版
    pub failed: Vec<i64>,
    /// 実行ファイル内のファイルと DB のチェックサムが食い違う版 (適用済みのファイルを書き換えた)
    pub checksum_mismatch: Vec<i64>,
    /// DB にはあるが実行ファイルには無い版 (新しい api が適用した後に古い api へ戻した)
    pub unknown: Vec<i64>,
    /// `_sqlx_migrations` テーブル自体が無い (一度もマイグレーションしていない DB)
    pub table_missing: bool,
}

impl MigrationStatus {
    /// 運用上そのままにしてはいけない状態か (画面で警告を出す)
    pub fn has_problem(&self) -> bool {
        self.table_missing
            || !self.pending.is_empty()
            || !self.failed.is_empty()
            || !self.checksum_mismatch.is_empty()
            || !self.unknown.is_empty()
    }
}

/// `_sqlx_migrations` と埋め込みのマイグレーションを突き合わせる
pub async fn migration_status<'e>(
    executor: impl PgExecutor<'e> + Copy,
) -> sqlx::Result<MigrationStatus> {
    let table_exists: bool = sqlx::query_scalar!(
        r#"SELECT (to_regclass('public._sqlx_migrations') IS NOT NULL) AS "exists!""#
    )
    .fetch_one(executor)
    .await?;

    let rows = if table_exists {
        sqlx::query!(
            r#"
            SELECT version, description, installed_on, success, checksum, execution_time
            FROM _sqlx_migrations
            ORDER BY version
            "#
        )
        .fetch_all(executor)
        .await?
    } else {
        Vec::new()
    };

    let embedded = sqlx::migrate!("./migrations");
    let mut pending = Vec::new();
    let mut checksum_mismatch = Vec::new();
    for migration in embedded.iter() {
        match rows.iter().find(|row| row.version == migration.version) {
            None => pending.push(PendingMigration {
                version: migration.version,
                description: migration.description.to_string(),
            }),
            Some(row) if row.checksum != migration.checksum.as_ref() => {
                checksum_mismatch.push(row.version);
            }
            Some(_) => {}
        }
    }
    let unknown = rows
        .iter()
        .filter(|row| !embedded.iter().any(|m| m.version == row.version))
        .map(|row| row.version)
        .collect();
    let failed = rows
        .iter()
        .filter(|row| !row.success)
        .map(|row| row.version)
        .collect();
    let latest = rows.last().map(|row| AppliedMigration {
        version: row.version,
        description: row.description.clone(),
        installed_on: row.installed_on,
        success: row.success,
        // sqlx はナノ秒で入れる
        execution_time_ms: row.execution_time / 1_000_000,
    });

    Ok(MigrationStatus {
        applied_count: rows.len() as i64,
        latest,
        pending,
        failed,
        checksum_mismatch,
        unknown,
        table_missing: !table_exists,
    })
}

/// PostgreSQL のバージョン文字列 (`SHOW server_version`)
pub async fn server_version<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<String> {
    sqlx::query_scalar::<_, String>("SHOW server_version")
        .fetch_one(executor)
        .await
}
