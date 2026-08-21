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

    // 旧実装の entrypoint が `sqlx migrate run` してから起動していたのと同じ振る舞い
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

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
