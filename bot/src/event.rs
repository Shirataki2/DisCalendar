//! Gateway イベントの処理 (旧 `event.rs` 相当)。
//!
//! `guilds` テーブルは web のサーバー選択が「Bot 参加済み」の判定に使うので、
//! Bot の参加・退出・ギルド名やアイコンの変更をここで反映する。
//! 定期タスクの起動 (旧 `CacheReady`) は #4、コマンドのログ (旧 `pre_command`) は #3 で移す。

use poise::serenity_prelude::{self as serenity, FullEvent};

use crate::{data::Data, error::BotError, models::guilds};

pub async fn handle_event(
    ctx: &serenity::Context,
    event: &FullEvent,
    _framework: poise::FrameworkContext<'_, Data, BotError>,
    data: &Data,
) -> Result<(), BotError> {
    match event {
        FullEvent::Ready { data_about_bot } => {
            tracing::info!(
                shard = ctx.shard_id.0,
                user = %data_about_bot.user.name,
                guilds = data_about_bot.guilds.len(),
                "shard ready"
            );
        }
        // 起動時に参加済みの各ギルドと、新しく参加したときに届く。
        // 起動時の分も upsert して、停止中に変わった名前・アイコンや取りこぼしを取り戻す
        FullEvent::GuildCreate { guild, is_new } => {
            let guild_id = guild.id.to_string();
            guilds::upsert(
                &data.pool,
                &guild_id,
                &guild.name,
                guild.icon_url().as_deref(),
            )
            .await?;
            // is_new は「Ready の時点で参加していなかったギルド」= 新規参加 (cache 機能が必要)
            if *is_new == Some(true) {
                tracing::info!(guild_id, name = %guild.name, "joined a guild");
                notify_log_channel(
                    ctx,
                    data,
                    "DisCalendar - New Guild",
                    0x0000ff,
                    format!("{} ({})", guild.name, guild_id),
                )
                .await;
            } else {
                tracing::debug!(guild_id, name = %guild.name, "registered guild");
            }
        }
        FullEvent::GuildUpdate { new_data, .. } => {
            let guild_id = new_data.id.to_string();
            guilds::upsert(
                &data.pool,
                &guild_id,
                &new_data.name,
                new_data.icon_url().as_deref(),
            )
            .await?;
            tracing::info!(guild_id, name = %new_data.name, "updated guild");
        }
        FullEvent::GuildDelete { incomplete, full } => {
            let guild_id = incomplete.id.to_string();
            // unavailable = true は Discord 側の障害でギルドが一時的に落ちただけ (退出ではない)
            if incomplete.unavailable {
                tracing::warn!(
                    guild_id,
                    "guild became unavailable (outage), keeping the row"
                );
                return Ok(());
            }
            let deleted = guilds::delete(&data.pool, &guild_id).await?;
            let name = full
                .as_ref()
                .map_or_else(|| "[unknown guild]".to_owned(), |g| g.name.clone());
            tracing::info!(guild_id, name = %name, deleted, "left a guild");
            notify_log_channel(
                ctx,
                data,
                "DisCalendar - Leave Guild",
                0x00ff00,
                format!("{name} ({guild_id})"),
            )
            .await;
        }
        _ => {}
    }
    Ok(())
}

/// 参加・退出を `BOT_LOG_CHANNEL_ID` のチャンネルに埋め込みで通知する (未設定なら何もしない)。
/// 通知の失敗で DB 更新まで失敗扱いにしないよう、エラーはログに出すだけにする
async fn notify_log_channel(
    ctx: &serenity::Context,
    data: &Data,
    title: &str,
    colour: u32,
    description: String,
) {
    let Some(channel_id) = data.log_channel_id else {
        return;
    };
    let embed = serenity::CreateEmbed::new()
        .title(title)
        .colour(colour)
        .description(description);
    if let Err(e) = channel_id
        .send_message(&ctx.http, serenity::CreateMessage::new().embed(embed))
        .await
    {
        tracing::warn!(error = %e, channel_id = channel_id.get(), "failed to send to log channel");
    }
}
