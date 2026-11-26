//! 保存期間を過ぎた本文を消す。フォーラムの投稿は解決から 180 日、テキストチャンネルの発言は投稿から 180 日。
//! 行とメタ情報 (投稿者・時刻・状態) は件数の振り返りのために残す。

use std::time::Duration;

use chrono::Utc;

use crate::{data::Data, db};

const INTERVAL: Duration = Duration::from_secs(60 * 60);
/// 本文の保存期間。変えるときはプライバシーポリシー (web/src/content/support/privacy.mdx) も直す
pub const RETENTION: chrono::Duration = chrono::Duration::days(180);

pub async fn run_loop(data: Data) {
    let mut interval = super::interval(INTERVAL);
    loop {
        interval.tick().await;
        match db::purge_expired(&data.pool, Utc::now(), RETENTION).await {
            Ok(purged) if purged == db::Purged::default() => {}
            Ok(purged) => tracing::info!(
                threads = purged.threads,
                messages = purged.messages,
                "purged expired support messages"
            ),
            Err(e) => tracing::warn!(error = %e, "failed to purge expired support messages"),
        }
        if let Err(e) = db::prune_tombstones(&data.pool, Utc::now()).await {
            tracing::warn!(error = %e, "failed to prune message tombstones");
        }
    }
}
