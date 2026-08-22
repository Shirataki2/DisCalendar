use std::sync::Arc;

use poise::serenity_prelude::ChannelId;
use sqlx::PgPool;
use tokio::sync::Mutex;

/// poise のユーザーデータ。全コマンド・イベントハンドラから `ctx.data()` で参照できる
#[derive(Debug, Clone)]
pub struct Data {
    pub pool: PgPool,
    /// Bot の参加・退出を通知するチャンネル (`BOT_LOG_CHANNEL_ID`)
    pub log_channel_id: Option<ChannelId>,
    /// `/help` と `/invite` で案内する Bot の招待 URL
    /// (`DISCORD_BOT_INVITE_URL`、未設定なら起動時にアプリケーション ID から組み立てる)
    pub invite_url: String,
    /// `guilds` テーブルへの書き込み (参加・更新時の upsert、退出時の削除、`reconcile_guilds` の突き合わせ) を直列化する。
    /// イベントハンドラは並行に走るので、ロックなしだと完了順がゲートウェイのイベント順と逆転して古い値が残ったり、
    /// 突き合わせの削除が参加し直した行を消したりする
    pub guild_sync: Arc<Mutex<()>>,
}

/// コマンドハンドラが受け取るコンテキスト
pub type Context<'a> = poise::Context<'a, Data, crate::error::BotError>;
