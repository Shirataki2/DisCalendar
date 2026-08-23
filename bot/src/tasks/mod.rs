//! 定期タスク (予定の通知 / presence 表示の切り替え / 日付入りアイコンへの更新)。
//!
//! notify / icon_updater はシャードに依存しない処理 (DB アクセスと HTTP 経由の更新) なので、
//! `event::handle_event` の最初の `FullEvent::Ready` から一度だけ起動する
//! (`Data::mark_tasks_started` で複数シャードでの二重起動を防ぐ)。全シャードが揃うのを待つ
//! `ShardsReady` だと、いずれか1シャードでも接続障害で `Ready` に到達しない間はこれらの
//! タスクが起動できず、待機中に発火するはずだった通知の判定窓を取りこぼしてしまうため、
//! シャード数が揃うのを待たない最初の `Ready` で起動する。
//! presence は `Context::set_presence` がそのシャードの接続にしか反映されないため、
//! 各シャードの `FullEvent::Ready` から個別に起動する (`spawn_presence`、
//! `Data::replace_presence_task` で再接続時に古いループを中断してから置き換える)。

mod icon_updater;
mod notify;
mod presence;

use poise::serenity_prelude as serenity;
use tokio::task::JoinHandle;

use crate::data::Data;

/// シャードに依存しない定期タスクをそれぞれ独立した tokio タスクとして起動する
pub fn spawn_all(ctx: serenity::Context, data: Data) {
    tokio::spawn(notify::run_loop(ctx.clone(), data.clone()));
    tokio::spawn(icon_updater::run_loop(ctx, data));
}

/// このシャードの接続に対して presence の切り替えループを起動する
pub fn spawn_presence(ctx: serenity::Context) -> JoinHandle<()> {
    tokio::spawn(presence::run_loop(ctx))
}
