//! 毎朝 9:00 (JST) に未解決チケットの一覧をスタッフ用チャンネルに投稿する。

use std::time::Duration;

use chrono::{DateTime, FixedOffset, NaiveTime, Utc};
use poise::serenity_prelude::{self as serenity, CreateEmbed};

use crate::{
    data::Data,
    db::{self, OpenThread},
    ticket::Status,
};

const INTERVAL: Duration = Duration::from_secs(60);
pub const TASK_NAME: &str = "daily_summary";
/// 投稿する時刻 (JST)
const POST_AT: NaiveTime = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

pub async fn run_loop(ctx: serenity::Context, data: Data) {
    let mut interval = super::interval(INTERVAL);
    loop {
        interval.tick().await;
        if let Err(error) = tick(&ctx, &data).await {
            tracing::warn!(%error, "failed to post the daily summary");
        }
    }
}

fn jst() -> FixedOffset {
    FixedOffset::east_opt(9 * 60 * 60).unwrap()
}

async fn tick(ctx: &serenity::Context, data: &Data) -> anyhow::Result<()> {
    let now = Utc::now().with_timezone(&jst());
    if now.time() < POST_AT {
        return Ok(());
    }
    let today = now.date_naive();
    // 投稿してから実行済みにする (再起動で二重に投稿しないよう DB に持つ。投稿に失敗したら次の周期で再試行)
    if !db::daily_task_pending(&data.pool, TASK_NAME, today).await? {
        return Ok(());
    }
    let threads = db::unresolved_threads(&data.pool).await?;
    // 未解決が無い日は投稿しない (毎朝「0 件」が流れるとチャンネルが埋もれるため)
    if !threads.is_empty() {
        let embed = summary_embed(&threads, now.to_utc());
        super::post_to_staff(ctx, data, None, embed, Vec::new()).await?;
        tracing::info!(count = threads.len(), "posted the daily summary");
    }
    db::mark_daily_task_done(&data.pool, TASK_NAME, today).await?;
    Ok(())
}

fn summary_embed(threads: &[OpenThread], now: DateTime<Utc>) -> CreateEmbed {
    let open = threads.iter().filter(|t| t.status == Status::Open).count();
    let waiting = threads.len() - open;
    let lines: Vec<String> = threads
        .iter()
        .map(|t| {
            // 未返信の通知と同じ基準を過ぎているものに印を付ける
            let overdue = t.status == Status::Open
                && t.last_user_msg_at
                    .is_some_and(|at| now - at >= super::unreplied::THRESHOLD);
            format!(
                "- {}{} {}",
                if overdue { "⚠️ " } else { "" },
                t.status.label(),
                super::thread_line(t.thread_id, t.title.as_deref())
            )
        })
        .collect();
    CreateEmbed::new()
        .title(format!(
            "📋 未解決のチケット {} 件 (対応待ち {open} / 返信待ち {waiting})",
            threads.len()
        ))
        .description(super::join_lines(&lines, 4000))
        .colour(0x5865f2)
}
