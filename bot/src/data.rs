use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use poise::serenity_prelude::{ChannelId, GuildId, ShardId};
use sqlx::PgPool;
use tokio::{sync::Mutex, task::JoinHandle};

/// poise のユーザーデータ。全コマンド・イベントハンドラから `ctx.data()` で参照できる
#[derive(Debug, Clone)]
pub struct Data {
    pub pool: PgPool,
    /// 通知とヘルプのリンク先 (`SITE_BASE_URL`)
    pub site_base_url: String,
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
    /// シャードごとに起動中の presence 切り替えループ。`Context::set_presence` はそのシャードの
    /// 接続にしか反映されないので、シャードごとに `Ready` イベントで起動する。
    /// serenity のシャードが re-identify を伴う再接続をすると同じ `ShardId` で改めて `Ready` が届くが、
    /// そのとき古いループは無効になった接続を握ったまま `set_presence` を送り続けてしまう
    /// (fire-and-forget で失敗が表面化しない)。`Ready` のたびに古いタスクを `abort` して
    /// 新しい `Context` のループに置き換えることで、再接続後も presence が更新され続けるようにする
    pub presence_tasks: Arc<Mutex<HashMap<ShardId, JoinHandle<()>>>>,
}

impl Data {
    /// 定期タスクをまだ起動していなければ true を返し、以降は false を返す (一度だけ起動するためのガード)
    pub fn mark_tasks_started(&self) -> bool {
        self.tasks_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// このシャードの presence ループを (既に動いていれば古いものを中断してから) 差し替える
    pub async fn replace_presence_task(&self, shard_id: ShardId, handle: JoinHandle<()>) {
        if let Some(old) = self.presence_tasks.lock().await.insert(shard_id, handle) {
            old.abort();
        }
    }
}

/// コマンドハンドラが受け取るコンテキスト
pub type Context<'a> = poise::Context<'a, Data, crate::error::BotError>;
