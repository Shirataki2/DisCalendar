//! DisCalendar Discord Bot。
//!
//! - Discord Gateway に接続し、Bot の参加・退出・ギルド情報の変更を `guilds` テーブルに反映する
//!   (web のサーバー選択 (`GET /guilds/joined`) が「Bot 参加済み」の判定に使う)
//! - スラッシュコマンド (#3) と定期タスク (通知 / presence / アイコン更新、#4) はこの上に載せる
//! - DB スキーマは api (`api/migrations/`) が正。Bot はマイグレーションを実行しない

pub mod config;
pub mod data;
pub mod error;
pub mod event;
pub mod models;

use anyhow::Context as _;
use poise::serenity_prelude as serenity;
use sqlx::postgres::PgPoolOptions;

use crate::{config::Config, data::Data, error::BotError};

/// DB 接続・poise フレームワークの構築・Gateway 接続までを行い、シャットダウンまで戻らない
pub async fn run(config: Config) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;
    tracing::info!("connected to database");

    let data = Data {
        pool,
        log_channel_id: config.log_channel_id,
    };

    let framework = poise::Framework::<Data, BotError>::builder()
        .options(poise::FrameworkOptions {
            // コマンドは #3 で追加する
            commands: vec![],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event::handle_event(ctx, event, framework, data))
            },
            on_error: |error| Box::pin(error::on_error(error)),
            ..Default::default()
        })
        .setup(move |_ctx, ready, _framework| {
            Box::pin(async move {
                tracing::info!(
                    user = %ready.user.name,
                    guilds = ready.guilds.len(),
                    "logged in to Discord"
                );
                Ok(data)
            })
        })
        .build();

    // 旧 Bot と同じく非特権インテントのみ。ギルドの参加・退出・更新には GUILDS が必要
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
