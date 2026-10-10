//! ゴミ箱 (#159) に入ってから保持期間 (30 日) を過ぎた予定を完全に消す。
//! 共有リンク・添付ファイル・変更履歴は `ON DELETE CASCADE` で一緒に消える
//! (添付ファイルの実体は api の後片付けが `attachment_deletions` を見て消す)。
//! 期限は日単位の目安なので、1 時間おきの粗い間隔でよい

use std::time::Duration;

use crate::{
    data::Data,
    models::{events, now_jst},
};

const INTERVAL: Duration = Duration::from_secs(60 * 60);

pub async fn run_loop(data: Data) {
    let mut interval = tokio::time::interval(INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match events::purge_expired_trash(&data.pool, now_jst()).await {
            Ok(0) => {}
            Ok(deleted) => tracing::info!(deleted, "purged expired events from the trash"),
            Err(e) => tracing::warn!(error = %e, "failed to purge expired events from the trash"),
        }
    }
}
