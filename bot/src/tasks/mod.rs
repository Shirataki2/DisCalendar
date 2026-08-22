//! 定期タスク (予定の通知 / presence 表示の切り替え / 日付入りアイコンへの更新)。
//!
//! `event::handle_event` の `FullEvent::ShardsReady` から一度だけ起動する
//! (`Data::mark_tasks_started` で autosharding 環境での二重起動を防ぐ)。

mod icon_updater;
mod notify;
mod presence;

use poise::serenity_prelude as serenity;

use crate::data::Data;

/// 3つの定期タスクをそれぞれ独立した tokio タスクとして起動する
pub fn spawn_all(ctx: serenity::Context, data: Data) {
    tokio::spawn(notify::run_loop(ctx.clone(), data.clone()));
    tokio::spawn(presence::run_loop(ctx.clone()));
    tokio::spawn(icon_updater::run_loop(ctx, data));
}
