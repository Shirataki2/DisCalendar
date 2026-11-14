//! Better Auth の期限切れ行 (`session` / `verification`) の定期削除 (#44)。
//!
//! Better Auth は期限切れのセッションを「そのセッションでアクセスがあったとき」にしか消さないので、
//! cookie が失効したまま戻ってこない利用者の行 (IP アドレスとユーザーエージェントを含む) が残り続ける。
//! `verification` (OAuth の state など、数分で失効する一時的な値) も同じく溜まる。
//! どちらも期限を過ぎた行は Better Auth 側でもう使われないので、消しても誰もログアウトされない。
//!
//! テーブルは web (Better Auth) の持ち物だが、api は同じ DB でセッションを検証しており、
//! 管理コンソールの定型操作 (`POST /admin/ops/purge-expired-sessions`) も api にあるので、ここで回す。
//! 期限は日単位 (セッションは最後の更新から 7 日) なので、1 時間おきの粗い間隔でよい

use std::time::Duration;

use sqlx::PgPool;

use crate::models::admin_ops;

/// 削除の間隔。期限切れの行が残るのは最長でこの時間
pub const INTERVAL: Duration = Duration::from_secs(60 * 60);

/// 起動直後に一度、以後 [`INTERVAL`] おきに期限切れ行を消す
pub async fn run(pool: PgPool) {
    let mut interval = tokio::time::interval(INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        // 新しい DB では web の起動 (Better Auth のマイグレーション) より前にテーブルが無いことがある。
        // その場合も警告だけ出して次の周期でやり直す
        match admin_ops::purge_expired_sessions(&pool).await {
            Ok(0) => {}
            Ok(deleted) => tracing::info!(deleted, "purged expired sessions"),
            Err(e) => tracing::warn!(error = %e, "failed to purge expired sessions"),
        }
        match admin_ops::purge_expired_verifications(&pool).await {
            Ok(0) => {}
            Ok(deleted) => tracing::info!(deleted, "purged expired verifications"),
            Err(e) => tracing::warn!(error = %e, "failed to purge expired verifications"),
        }
    }
}
