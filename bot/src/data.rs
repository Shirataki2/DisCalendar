use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use poise::serenity_prelude::{ChannelId, GuildId, ShardId};
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
    /// 日付入りアイコンを反映するサポートサーバー (`BOT_SUPPORT_GUILD_ID`)。未設定なら更新しない
    pub support_guild_id: Option<GuildId>,
    /// `guilds` テーブルへの書き込み (参加・更新時の upsert、退出時の削除、`reconcile_guilds` の突き合わせ) を直列化する。
    /// イベントハンドラは並行に走るので、ロックなしだと完了順がゲートウェイのイベント順と逆転して古い値が残ったり、
    /// 突き合わせの削除が参加し直した行を消したりする
    pub guild_sync: Arc<Mutex<()>>,
    /// 定期タスク (`tasks::spawn_all`、シャードに依存しない notify / icon_updater) を起動済みかどうか。
    /// `ShardsReady` は autosharding 環境で複数回発火する可能性があるため、最初の1回だけ起動するようここで防ぐ
    pub tasks_started: Arc<AtomicBool>,
    /// presence の切り替えループを起動済みのシャード ID。`Context::set_presence` はそのシャードの接続にしか
    /// 反映されないので、シャードごとに `Ready` イベントで起動する。1つのシャードで複数回起動しないためのガード
    pub presence_started_shards: Arc<Mutex<HashSet<ShardId>>>,
}

impl Data {
    /// 定期タスクをまだ起動していなければ true を返し、以降は false を返す (一度だけ起動するためのガード)
    pub fn mark_tasks_started(&self) -> bool {
        self.tasks_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// このシャードで presence ループをまだ起動していなければ true を返す (一度だけ起動するためのガード)
    pub async fn mark_presence_started(&self, shard_id: ShardId) -> bool {
        self.presence_started_shards.lock().await.insert(shard_id)
    }
}

/// コマンドハンドラが受け取るコンテキスト
pub type Context<'a> = poise::Context<'a, Data, crate::error::BotError>;
