use std::num::NonZeroU64;

use anyhow::{Context as _, Result};
use poise::serenity_prelude::ChannelId;

/// 環境変数から読む設定。開発時は `bot/.env` (dotenvy) から読み込まれる
#[derive(Debug, Clone)]
pub struct Config {
    /// Discord Bot トークン (api の `DISCORD_BOT_TOKEN` と同じ値)
    pub discord_bot_token: String,
    pub database_url: String,
    pub max_db_connections: u32,
    /// Bot の参加・退出を通知するチャンネル。未設定なら通知しない
    pub log_channel_id: Option<ChannelId>,
    /// `/help` と `/invite` で案内する招待 URL (web の `DISCORD_BOT_INVITE_URL` と同じ)。
    /// 未設定なら起動時にアプリケーション ID から組み立てる
    pub invite_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let log_channel_id = optional("BOT_LOG_CHANNEL_ID")
            .map(|v| {
                v.parse::<NonZeroU64>()
                    .map(ChannelId::from)
                    .context("BOT_LOG_CHANNEL_ID must be a channel ID (snowflake)")
            })
            .transpose()?;
        Ok(Self {
            discord_bot_token: required("DISCORD_BOT_TOKEN")?,
            database_url: required("DATABASE_URL")?,
            max_db_connections: env_or("DATABASE_MAX_CONNECTIONS", "5")
                .parse()
                .context("DATABASE_MAX_CONNECTIONS must be a number")?,
            log_channel_id,
            invite_url: optional("DISCORD_BOT_INVITE_URL"),
        })
    }
}

fn required(key: &str) -> Result<String> {
    optional(key).with_context(|| format!("{key} must be set"))
}

fn optional(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

fn env_or(key: &str, default: &str) -> String {
    optional(key).unwrap_or_else(|| default.to_owned())
}
