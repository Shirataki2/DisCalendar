//! DisCalendar サポート用 Bot (#423)。
//!
//! サポートサーバーにだけ入る、本体とは別の Discord アプリ。本体の Bot は全ギルドに入るので
//! MESSAGE_CONTENT 特権インテントを持たせず、本文が要る問い合わせ対応はこちらで受ける。
//!
//! - 記録: 許可リストのチャンネル (フォーラムの投稿 = チケット、テキストチャンネル) の発言・編集・削除と、投稿の状態
//! - 定期タスク: 未返信の通知、`/remind` のリマインド、毎朝の未解決まとめ、保存期間を過ぎた本文の削除
//! - コマンド: `/ticket status`、`/remind` (スタッフのみ)
//! - DB は本体とは別のデータベース。マイグレーション (`support-bot/migrations/`) は起動時にこの Bot が当てる

pub mod backfill;
pub mod commands;
pub mod config;
pub mod data;
pub mod db;
pub mod error;
pub mod event;
pub mod record;
pub mod tasks;
pub mod ticket;

use std::str::FromStr as _;

use anyhow::Context as _;
use poise::serenity_prelude as serenity;
use sqlx::{
    ConnectOptions as _, Connection as _,
    postgres::{PgConnectOptions, PgPoolOptions},
};

use crate::{config::Config, data::Data, error::BotError};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// DB の準備・poise フレームワークの構築・Gateway 接続までを行い、シャットダウンまで戻らない
pub async fn run(config: Config) -> anyhow::Result<()> {
    ensure_database(&config.database_url).await?;
    let pool = PgPoolOptions::new()
        .max_connections(config.max_db_connections)
        .connect(&config.database_url)
        .await
        .context("failed to connect to database")?;
    // 単一インスタンスで動かす前提なので、sqlx 自身のロック (pg_advisory_lock) に任せる
    MIGRATOR
        .run(&pool)
        .await
        .context("failed to run migrations")?;
    tracing::info!("connected to database and applied migrations");
    // 補完の起点は Gateway に繋ぐ前に固定する (繋いだあとの新着を記録すると最大 ID が進み、停止中の発言を飛ばすため)
    let backfill_from = std::sync::Arc::new(
        db::last_message_ids(&pool)
            .await
            .context("failed to read the last recorded messages")?,
    );

    let settings = std::sync::Arc::new(config.settings.clone());
    let guild_id = settings.guild_id;
    let framework = poise::Framework::<Data, BotError>::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all(),
            // スラッシュコマンドだけを使う。MESSAGE_CONTENT があると poise は既定で Bot へのメンションを
            // プレフィックスとして読み、「@Bot register」などに unknown command の警告を出すので止める
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: None,
                mention_as_prefix: false,
                ..Default::default()
            },
            event_handler: |ctx, event, framework, data| {
                Box::pin(event::handle_event(ctx, event, framework, data))
            },
            on_error: |error| Box::pin(error::on_error(error)),
            ..Default::default()
        })
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                tracing::info!(
                    user = %ready.user.name,
                    application_id = ready.application.id.get(),
                    "logged in to Discord"
                );
                // サポートサーバー 1 か所でしか使わないので、反映の速いギルドコマンドとして毎回登録する
                poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id)
                    .await?;
                Ok(Data {
                    pool,
                    settings,
                    tasks_started: Default::default(),
                    backfill_from,
                })
            })
        })
        .build();

    // 本文 (MESSAGE_CONTENT) は特権インテント。サポートサーバー 1 か所にしか入らない Bot なので審査なしで使える。
    // GUILD_MEMBERS は使わない (スタッフの判定は発言に付いてくるメンバー情報のロールで行う)
    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;
    let mut client = serenity::ClientBuilder::new(&config.discord_bot_token, intents)
        .framework(framework)
        .await
        .context("failed to create Discord client")?;

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        tracing::info!("shutdown signal received, closing shards");
        shard_manager.shutdown_all().await;
    });

    // 1 ギルドしか入らないのでシャードは 1 本
    client.start().await.context("Discord client error")
}

/// 接続先のデータベースが無ければ作る。本体とは別のデータベースを使うので、既存の本番環境
/// (initdb を済ませた Postgres) に後から足したときも手作業なしで起動できるようにする。
/// 作成には同じホストの `postgres` データベースに接続し、接続ユーザーに CREATEDB 権限が要る
/// (compose の POSTGRES_USER はスーパーユーザー)
pub async fn ensure_database(database_url: &str) -> anyhow::Result<()> {
    let options = PgConnectOptions::from_str(database_url).context("invalid DATABASE_URL")?;
    let Some(name) = options.get_database().map(str::to_owned) else {
        return Ok(());
    };
    anyhow::ensure!(
        is_plain_identifier(&name),
        "database name must consist of lowercase letters, digits and underscores: {name}"
    );
    let mut conn = options
        .clone()
        .database("postgres")
        .connect()
        .await
        .context("failed to connect to the postgres maintenance database")?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&name)
            .fetch_one(&mut conn)
            .await?;
    if !exists {
        // 識別子はバインドできないので、上で文字種を確かめた名前をそのまま埋め込む
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
            .execute(&mut conn)
            .await
            .with_context(|| format!("failed to create database {name}"))?;
        tracing::info!(database = %name, "created the support database");
    }
    conn.close().await.ok();
    Ok(())
}

fn is_plain_identifier(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_plain_database_names() {
        assert!(is_plain_identifier("discalendar_support"));
        assert!(!is_plain_identifier("Support"));
        assert!(!is_plain_identifier("a;DROP"));
        assert!(!is_plain_identifier("1db"));
        assert!(!is_plain_identifier(""));
    }
}
