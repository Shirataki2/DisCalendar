use poise::serenity_prelude::ChannelId;
use sqlx::PgPool;

/// poise のユーザーデータ。全コマンド・イベントハンドラから `ctx.data()` で参照できる
#[derive(Debug, Clone)]
pub struct Data {
    pub pool: PgPool,
    /// Bot の参加・退出を通知するチャンネル (`BOT_LOG_CHANNEL_ID`)
    pub log_channel_id: Option<ChannelId>,
}

/// コマンドハンドラが受け取るコンテキスト (#3 で使う)
pub type Context<'a> = poise::Context<'a, Data, crate::error::BotError>;
