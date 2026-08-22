use std::sync::Arc;

use poise::serenity_prelude::ChannelId;
use sqlx::PgPool;
use tokio::sync::RwLock;

/// poise のユーザーデータ。全コマンド・イベントハンドラから `ctx.data()` で参照できる
#[derive(Debug, Clone)]
pub struct Data {
    pub pool: PgPool,
    /// Bot の参加・退出を通知するチャンネル (`BOT_LOG_CHANNEL_ID`)
    pub log_channel_id: Option<ChannelId>,
    /// `guilds` テーブルの突き合わせ (`reconcile_guilds`、write)・退出時の削除 (write) と参加・更新時の upsert (read) を
    /// 直列化する。一覧取得から削除までの間に参加し直したギルドの行を消したり、削除と upsert が重なったりしないため
    pub guild_sync: Arc<RwLock<()>>,
}

/// コマンドハンドラが受け取るコンテキスト (#3 で使う)
pub type Context<'a> = poise::Context<'a, Data, crate::error::BotError>;
