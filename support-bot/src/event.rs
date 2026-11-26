//! Gateway イベントの処理。サポートサーバー以外のギルドには居座らず、許可リストのチャンネルだけを記録する。

use poise::serenity_prelude::{self as serenity, FullEvent};

use crate::{
    backfill,
    data::Data,
    db,
    error::BotError,
    record::{self, snowflake},
    tasks, ticket,
};

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
            if data.mark_tasks_started() {
                // 定期タスクは補完が終わってから始める。先に始めると、停止中の発言や解決を反映する前の DB で
                // 毎朝のまとめを送ってその日を実行済みにしたり、解決済みのチケットを未返信として通知したりする
                let (ctx, data) = (ctx.clone(), data.clone());
                tokio::spawn(async move {
                    backfill::run(ctx.clone(), data.clone()).await;
                    tracing::info!("starting periodic tasks");
                    tasks::spawn_all(ctx, data);
                });
            }
        }
        // 起動時に参加済みの各ギルドと、新しく参加したときに届く。
        // Public Bot をオフにしていても、管理者が誤って招待する可能性はあるので、サポートサーバー以外からは抜ける
        FullEvent::GuildCreate { guild, .. } if guild.id != data.settings.guild_id => {
            tracing::warn!(guild_id = guild.id.get(), name = %guild.name, "joined an unexpected guild, leaving");
            if let Err(error) = guild.id.leave(ctx).await {
                tracing::error!(%error, guild_id = guild.id.get(), "failed to leave an unexpected guild");
            }
        }

        FullEvent::Message { new_message } => {
            if new_message.guild_id != Some(data.settings.guild_id)
                || !record::is_recordable(new_message)
            {
                return Ok(());
            }
            let Some(scope) = record::scope_of(ctx, data, new_message.channel_id).await? else {
                return Ok(());
            };
            // ギルドの発言には送信者のメンバー情報 (ロール) が付いてくるので、GUILD_MEMBERS インテントは要らない
            let is_staff = new_message
                .member
                .as_ref()
                .is_some_and(|member| data.is_staff(&member.roles));
            record::record(ctx, data, new_message, scope, is_staff).await?;
        }
        FullEvent::MessageUpdate { event, .. } => {
            if event.guild_id != Some(data.settings.guild_id) {
                return Ok(());
            }
            // 本文の変更を伴わない更新 (埋め込みの展開など) は content が来ないので、届いた分だけ上書きする
            if event.content.is_none() && event.attachments.is_none() {
                return Ok(());
            }
            // 記録済みのメッセージでも、チャンネルや親フォーラムを許可リストから外していれば取り込まない
            if !record::is_recorded_channel(data, event.channel_id).await? {
                return Ok(());
            }
            let attachments = event.attachments.as_deref().map(record::attachments);
            let edited_at = event
                .edited_timestamp
                .map(record::to_utc)
                .unwrap_or_else(chrono::Utc::now);
            db::update_message(
                &data.pool,
                snowflake(event.id),
                event.content.as_deref(),
                attachments.as_deref(),
                edited_at,
            )
            .await?;
        }
        FullEvent::MessageDelete {
            deleted_message_id,
            guild_id,
            ..
        } if *guild_id == Some(data.settings.guild_id) => {
            db::mark_messages_deleted(&data.pool, &[snowflake(*deleted_message_id)]).await?;
        }
        FullEvent::MessageDeleteBulk {
            multiple_deleted_messages_ids,
            guild_id,
            ..
        } if *guild_id == Some(data.settings.guild_id) => {
            let ids: Vec<i64> = multiple_deleted_messages_ids
                .iter()
                .map(|id| snowflake(*id))
                .collect();
            db::mark_messages_deleted(&data.pool, &ids).await?;
        }

        FullEvent::ThreadCreate { thread } => {
            if let Some(input) = record::ticket_thread(ctx, data, thread).await {
                tracing::info!(thread_id = thread.id.get(), "new support thread");
                db::upsert_thread(&data.pool, &input).await?;
            }
        }
        FullEvent::ThreadUpdate { old, new } => {
            let Some(input) = record::ticket_thread(ctx, data, new).await else {
                return Ok(());
            };
            db::upsert_thread(&data.pool, &input).await?;
            sync_status_from_tags(ctx, data, old.as_ref(), new).await?;
        }
        FullEvent::ThreadDelete { thread, .. } if thread.guild_id == data.settings.guild_id => {
            let deleted = db::mark_thread_deleted(&data.pool, snowflake(thread.id)).await?;
            tracing::debug!(thread_id = thread.id.get(), deleted, "thread deleted");
        }
        _ => {}
    }
    Ok(())
}

/// Discord 側で「解決済み」タグが付け外しされたら、DB の状態を合わせる。
/// タグが変わったときだけ見る (タグの数の上限で Bot がタグを付けられなかった解決済みのスレッドを、
/// スレッド名の変更などのたびに再オープンしないため)。変更前がキャッシュに無ければ判断しない
async fn sync_status_from_tags(
    ctx: &serenity::Context,
    data: &Data,
    old: Option<&serenity::GuildChannel>,
    new: &serenity::GuildChannel,
) -> Result<(), BotError> {
    let (Some(old), Some(parent_id)) = (old, new.parent_id) else {
        return Ok(());
    };
    let Some(available) = ticket::forum_tags(ctx, new.guild_id, parent_id).await else {
        return Ok(());
    };
    let was = ticket::has_resolved_tag(&available, &old.applied_tags);
    let now = ticket::has_resolved_tag(&available, &new.applied_tags);
    if was == now {
        return Ok(());
    }
    let thread_id = snowflake(new.id);
    let current = db::thread_status(&data.pool, thread_id).await?;
    let target = match (now, current) {
        (true, Some(status)) if status != ticket::Status::Resolved => {
            Some(ticket::Status::Resolved)
        }
        (false, Some(ticket::Status::Resolved)) => None,
        _ => return Ok(()),
    };
    if let Some(transition) = db::set_status(&data.pool, thread_id, target).await? {
        tracing::info!(
            thread_id,
            before = transition.before.as_db(),
            after = transition.after.as_db(),
            "ticket status changed by forum tags"
        );
    }
    Ok(())
}
