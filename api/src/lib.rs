//! DisCalendar REST API。
//!
//! - 認証: web (Next.js + Better Auth) が発行するセッション cookie を検証し、
//!   共有 Postgres の `session` / `account` テーブルから Discord ユーザー ID を引く
//! - 認可: Bot トークンで Discord API からメンバーシップとロールを取得して権限を計算する
//! - データ: 旧実装と同じ `guilds` / `events` / `guild_config` テーブル (Bot と共有)

pub mod auth;
pub mod config;
pub mod discord;
pub mod error;
pub mod models;
pub mod openapi;
pub mod routes;
pub mod state;

use actix_web::{App, HttpServer, middleware, web};
use anyhow::Context as _;
use sqlx::postgres::PgPoolOptions;
use tracing_actix_web::TracingLogger;
use utoipa::OpenApi as _;
use utoipa_actix_web::AppExt as _;
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    auth::AuthConfig, config::Config, discord::DiscordClient, error::ApiError, openapi::ApiDoc,
    state::AppState,
};

/// DB 接続・マイグレーション・HTTP サーバー起動までを行う
pub async fn run(config: Config) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;

    run_startup_migrations(&pool).await?;

    let state = web::Data::new(AppState {
        pool,
        discord: DiscordClient::new(&config.discord_bot_token)?,
        auth: AuthConfig {
            secret: config.better_auth_secret.clone(),
            cookie_names: config.session_cookie_names().to_vec(),
        },
    });

    let addr = (config.host.as_str(), config.port);
    tracing::info!(host = %config.host, port = config.port, "starting DisCalendar API");

    HttpServer::new(move || {
        App::new()
            .into_utoipa_app()
            .openapi(ApiDoc::openapi())
            .map(|app| {
                app.wrap(TracingLogger::default())
                    .wrap(middleware::Compress::default())
            })
            .app_data(state.clone())
            .app_data(json_config())
            .app_data(path_config())
            .app_data(query_config())
            .configure(routes::configure)
            .openapi_service(|api| SwaggerUi::new("/docs/{_:.*}").url("/openapi.json", api))
            .into_app()
    })
    .bind(addr)
    .with_context(|| format!("failed to bind {}:{}", config.host, config.port))?
    .run()
    .await
    .context("server error")
}

/// 掃除からマイグレーション完了までを跨いで直列化するアドバイザリロックの ID。
/// sqlx が `Migrator::run` の内部で使うロック (データベース名の CRC32 から算出、
/// `sqlx-postgres` の非公開関数なのでここでは再現できない) とは別物で、値そのものに
/// 意味はなく他で使われていなければよい。
///
/// 複数の API インスタンスが同時にこのバージョンへ更新される場合、掃除とマイグレーションが
/// このロックなしでは直列化されない。一方が `CREATE INDEX CONCURRENTLY` を実行中の
/// 正常な中間状態 (`indisvalid = false`) を、もう一方の掃除が「失敗残骸」と誤認して
/// 削除してしまうと、削除完了を待ってから有効になったインデックスが消え、その後
/// 前者はマイグレーションを成功として記録してしまうため、二度と再作成されなくなる
const STARTUP_LOCK_ID: i64 = 8_612_004;

/// 無効なインデックスの掃除とマイグレーションの適用を、複数の API インスタンス間で
/// 直列化しながら行う
async fn run_startup_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let mut lock_conn = pool
        .acquire()
        .await
        .context("failed to acquire a connection for the startup lock")?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(STARTUP_LOCK_ID)
        .execute(&mut *lock_conn)
        .await
        .context("failed to acquire the startup advisory lock")?;

    let result = async {
        cleanup_invalid_concurrent_indexes(pool).await?;
        // 旧実装の entrypoint が `sqlx migrate run` してから起動していたのと同じ振る舞い
        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .context("failed to run migrations")
    }
    .await;

    // pg_advisory_lock はセッションスコープなので、ロック取得と同じコネクションで解放する
    if let Err(e) = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(STARTUP_LOCK_ID)
        .execute(&mut *lock_conn)
        .await
    {
        tracing::warn!(error = %e, "failed to release the startup advisory lock");
    }

    result
}

/// `CREATE INDEX CONCURRENTLY` (`migrations/..._create_events_start_at_index_concurrently.sql`)
/// が接続断・キャンセルなどで失敗すると、同名の `INVALID` なインデックスが残ることがある。
/// マイグレーション本体は一度成功すると `_sqlx_migrations` に記録され二度と実行されないため、
/// 掃除をマイグレーションの中に書くと、無効なインデックスが残ったまま
/// `IF NOT EXISTS` がそれをスキップし続け、次に起動しても直せない。
/// マイグレーション実行の直前に毎回この掃除を試みることで、失敗がいつ起きても
/// 次の起動で確実に作り直せるようにする。
///
/// `DROP INDEX` (非 CONCURRENTLY) は `events` への排他ロックを取るため、稼働中の
/// 旧 Bot / API による予定の作成・更新を削除完了までブロックしてしまう。せっかく作成側を
/// CONCURRENTLY にしても復旧経路で同じ停止が起きては意味がないので、DROP も
/// CONCURRENTLY で行う (トランザクション内では実行できないため DO ブロックは使わず、
/// 先に存在確認してから条件付きで実行する)。呼び出し元 (`run_startup_migrations`) が
/// アドバイザリロックで直列化しているので、他インスタンスが構築中の有効なインデックスを
/// 誤って消すことはない
async fn cleanup_invalid_concurrent_indexes(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let has_invalid_index: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM pg_class c
            JOIN pg_index i ON i.indexrelid = c.oid
            WHERE c.relname = 'idx_events_start_at' AND NOT i.indisvalid
        )
        "#,
    )
    .fetch_one(pool)
    .await
    .context("failed to check for an invalid idx_events_start_at index")?;

    if !has_invalid_index {
        return Ok(());
    }
    tracing::warn!("found an invalid idx_events_start_at index, dropping it concurrently");
    sqlx::query("DROP INDEX CONCURRENTLY IF EXISTS idx_events_start_at")
        .execute(pool)
        .await
        .context("failed to drop an invalid idx_events_start_at index")?;
    Ok(())
}

// 抽出エラー (不正な JSON / パス / クエリ) も JSON のエラーレスポンスに統一する

fn json_config() -> web::JsonConfig {
    web::JsonConfig::default()
        .limit(64 * 1024)
        .error_handler(|err, _| ApiError::BadRequest(err.to_string()).into())
}

fn path_config() -> web::PathConfig {
    web::PathConfig::default().error_handler(|err, _| ApiError::BadRequest(err.to_string()).into())
}

fn query_config() -> web::QueryConfig {
    web::QueryConfig::default().error_handler(|err, _| ApiError::BadRequest(err.to_string()).into())
}
