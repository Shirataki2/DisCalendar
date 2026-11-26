//! サポートの記録を読む読み取り専用の MCP サーバー (stdio)。#425
//!
//! Claude Code から起動する。接続先は環境変数で受け取る (`.env` は読まない。support-bot/.env の
//! `DATABASE_URL` は Bot 用の書き込める接続なので、取り違えないよう名前も分けている):
//!
//! - `SUPPORT_MCP_DATABASE_URL` (必須): サポート用 DB に読み取り専用のロールで繋ぐ URL
//! - `SUPPORT_MCP_MAIN_DATABASE_URL` (任意): 本体 DB。あれば `get_guild` を出す

use anyhow::Context as _;
use discalendar_support_bot::mcp::{self, Tools};
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // stdout は MCP の応答専用。ログは stderr に出す (Claude Code は stderr を MCP のログとして扱う)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .init();

    let support_url = std::env::var("SUPPORT_MCP_DATABASE_URL")
        .context("SUPPORT_MCP_DATABASE_URL must be set")?;
    let support = mcp::connect_read_only(&support_url, "support").await?;
    let main = match std::env::var("SUPPORT_MCP_MAIN_DATABASE_URL") {
        Ok(url) if !url.trim().is_empty() => Some(mcp::connect_read_only(&url, "main").await?),
        _ => None,
    };
    tracing::info!(main_database = main.is_some(), "support MCP server started");

    let tools = Tools { support, main };
    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    mcp::serve(&tools, stdin, tokio::io::stdout()).await
}
