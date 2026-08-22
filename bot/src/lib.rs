//! DisCalendar Discord Bot。
//!
//! - Discord Gateway に接続し、Bot の参加・退出・ギルド情報の変更を `guilds` テーブルに反映する
//!   (web のサーバー選択 (`GET /guilds/joined`) が「Bot 参加済み」の判定に使う)
//! - スラッシュコマンド (`commands`): help / create / list / init / invite と、オーナー専用の register
//! - 定期タスク (通知 / presence / アイコン更新、#4) はこの上に載せる
//! - DB スキーマは api (`api/migrations/`) が正。Bot はマイグレーションを実行しない

pub mod checks;
pub mod commands;
pub mod config;
pub mod data;
pub mod error;
pub mod event;
pub mod models;
pub mod paginator;
pub mod tasks;

use anyhow::Context as _;
use poise::serenity_prelude as serenity;
use sqlx::postgres::PgPoolOptions;

use crate::{config::Config, data::Data, error::BotError};

/// `/help` のフッターに出すバージョン
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// DB 接続・poise フレームワークの構築・Gateway 接続までを行い、シャットダウンまで戻らない
pub async fn run(config: Config) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;
    tracing::info!("connected to database");

    let log_channel_id = config.log_channel_id;
    let invite_url = config.invite_url.clone();
    let support_guild_id = config.support_guild_id;

    let framework = poise::Framework::<Data, BotError>::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            // プレフィックスコマンドは `register` だけで、Bot へのメンション (`@DisCalendar register`) で呼ぶ。
            // 旧版の `cal` プレフィックスは MESSAGE_CONTENT 特権インテントがないと本文が届かないので設けない
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: None,
                mention_as_prefix: true,
                ..Default::default()
            },
            pre_command: |ctx| Box::pin(commands::log_invocation(ctx)),
            // `owners_only` (register) の判定に使う `owners` は、起動時に poise が
            // `GET /oauth2/applications/@me` からアプリの所有者 (チームなら Admin / Developer) を入れる
            initialize_owners: true,
            event_handler: |ctx, event, framework, data| {
                Box::pin(event::handle_event(ctx, event, framework, data))
            },
            on_error: |error| Box::pin(error::on_error(error)),
            ..Default::default()
        })
        .setup(move |_ctx, ready, _framework| {
            Box::pin(async move {
                // 招待 URL は環境変数で上書きできる。未設定なら Discord が返すアプリケーション ID から組み立てる
                let invite_url = invite_url
                    .unwrap_or_else(|| commands::invite::default_invite_url(ready.application.id));
                tracing::info!(
                    user = %ready.user.name,
                    application_id = ready.application.id.get(),
                    guilds = ready.guilds.len(),
                    "logged in to Discord"
                );
                Ok(Data {
                    pool,
                    log_channel_id,
                    invite_url,
                    support_guild_id,
                    guild_sync: Default::default(),
                    tasks_started: Default::default(),
                })
            })
        })
        .build();

    // 旧 Bot と同じく非特権インテントのみ。ギルドの参加・退出・更新には GUILDS が、
    // メンションでの `register` には GUILD_MESSAGES が必要 (どちらも non_privileged に含まれる)
    let intents = serenity::GatewayIntents::non_privileged();
    let mut client = serenity::ClientBuilder::new(&config.discord_bot_token, intents)
        .framework(framework)
        .await
        .context("failed to create Discord client")?;

    // SIGINT / SIGTERM (docker stop) で Gateway 接続を閉じてから終了する
    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::info!("shutdown signal received, closing shards");
        shard_manager.shutdown_all().await;
    });

    client
        .start_autosharded()
        .await
        .context("Discord client error")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %e, "failed to listen for SIGINT");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
}
