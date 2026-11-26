//! 利用者の最終発言から 24 時間スタッフの返信がないチケットを、スタッフ用チャンネルに知らせる。

use std::time::Duration;

use chrono::Utc;
use poise::serenity_prelude::{self as serenity, CreateEmbed};

use crate::{data::Data, db};

const INTERVAL: Duration = Duration::from_secs(10 * 60);
/// 未返信とみなすまでの時間
pub const THRESHOLD: chrono::Duration = chrono::Duration::hours(24);

pub async fn run_loop(ctx: serenity::Context, data: Data) {
    let mut interval = super::interval(INTERVAL);
    loop {
        interval.tick().await;
        if let Err(error) = tick(&ctx, &data).await {
            tracing::warn!(%error, "failed to notify unreplied tickets");
        }
    }
}

async fn tick(ctx: &serenity::Context, data: &Data) -> anyhow::Result<()> {
    let now = Utc::now();
    let threads = db::unreplied_threads(&data.pool, now, THRESHOLD).await?;
    if threads.is_empty() {
        return Ok(());
    }
    let lines: Vec<String> = threads
        .iter()
        .map(|t| {
            let since = t.last_user_msg_at.map_or(String::new(), |at| {
                format!(" (最終発言 <t:{}:R>)", at.timestamp())
            });
            format!(
                "- {}{since}",
                super::thread_line(t.thread_id, t.title.as_deref())
            )
        })
        .collect();
    let (description, shown) = super::join_lines_counted(&lines, 4000);
    let embed = CreateEmbed::new()
        .title(format!(
            "⏳ 24 時間以上返信していないチケット ({} 件)",
            threads.len()
        ))
        .description(description)
        .colour(0xf0b232);
    super::post_to_staff(ctx, data, None, embed, Vec::new()).await?;
    // 投稿できて、本文に載せたものだけ通知済みにする。投稿に失敗したときや、文字数の上限で
    // 「ほか N 件」に省いたものは、次の周期 (古い順なので省いた分が先頭に来る) でもう一度送る
    let ids: Vec<i64> = threads[..shown].iter().map(|t| t.thread_id).collect();
    db::mark_reminded(&data.pool, &ids, now).await?;
    tracing::info!(count = ids.len(), "notified unreplied tickets");
    Ok(())
}
