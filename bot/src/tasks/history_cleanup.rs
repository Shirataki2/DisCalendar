//! 予定の変更履歴 (#165) のうち、保持期間 (180 日) を過ぎたものを消す。
//! 予定ごとの件数の上限 (100 件) は書き込み時に api / bot の `event_history::record` が守るので、ここでは日数だけを見る。
//! 履歴は表示にしか使わないので、1 時間おきの粗い間隔でよい

use std::time::Duration;

use crate::data::Data;

const INTERVAL: Duration = Duration::from_secs(60 * 60);

pub async fn run_loop(data: Data) {
    let mut interval = tokio::time::interval(INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match crate::event_history::prune_expired(&data.pool).await {
            Ok(0) => {}
            Ok(deleted) => tracing::info!(deleted, "pruned expired event history"),
            Err(e) => tracing::warn!(error = %e, "failed to prune expired event history"),
        }
    }
}
