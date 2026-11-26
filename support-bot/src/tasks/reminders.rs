//! `/remind` で設定したリマインドを、期限が来たらスタッフ用チャンネルに知らせる。

use std::time::Duration;

use chrono::Utc;
use poise::serenity_prelude::{self as serenity, CreateEmbed, UserId};

use crate::{data::Data, db};

const INTERVAL: Duration = Duration::from_secs(60);

pub async fn run_loop(ctx: serenity::Context, data: Data) {
    let mut interval = super::interval(INTERVAL);
    loop {
        interval.tick().await;
        if let Err(error) = tick(&ctx, &data).await {
            tracing::warn!(%error, "failed to send reminders");
        }
    }
}

async fn tick(ctx: &serenity::Context, data: &Data) -> anyhow::Result<()> {
    for reminder in db::due_reminders(&data.pool, Utc::now()).await? {
        let mut description = super::thread_line(reminder.thread_id, reminder.title.as_deref());
        if let Some(note) = &reminder.note {
            description.push_str("\nメモ: ");
            description.push_str(&super::sanitize(note, 200));
        }
        let embed = CreateEmbed::new()
            .title("⏰ リマインド")
            .description(description)
            .colour(0x5865f2);
        // 設定した本人にだけメンションする
        let creator = UserId::new(reminder.created_by.cast_unsigned());
        super::post_to_staff(
            ctx,
            data,
            Some(format!("<@{}>", creator.get())),
            embed,
            vec![creator],
        )
        .await?;
        db::mark_reminder_sent(&data.pool, reminder.id).await?;
    }
    Ok(())
}
