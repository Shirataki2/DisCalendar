//! 起動時の取りこぼし補完。Bot が止まっていた間の発言を、記録済みの最後のメッセージ以降から取り直す
//! (取りこぼすと「利用者が書いたのに未返信の通知が出ない」状態になるため)。
//!
//! - フォーラム: アクティブな投稿ごとに、記録済みの続きから最大 100 件 (未登録の投稿は直近 100 件)
//! - テキストチャンネル: 記録済みの続きから最大 100 件。一度も記録していないチャンネルは、導入前の
//!   発言まで取り込まないよう補完しない
//!
//! 100 件を超えて取りこぼした分は諦める (長時間止めたときの負荷と、Discord の rate limit を避けるため)。

use std::collections::HashMap;

use poise::serenity_prelude::{self as serenity, ChannelType, GetMessages, MessageId, UserId};

use crate::{
    data::Data,
    db,
    record::{self, Scope, snowflake},
};

const LIMIT: u8 = 100;

pub async fn run(ctx: serenity::Context, data: Data) {
    let mut staff = StaffLookup::default();
    if let Err(error) = channels(&ctx, &data, &mut staff).await {
        tracing::warn!(%error, "failed to backfill text channels");
    }
    if let Err(error) = threads(&ctx, &data, &mut staff).await {
        tracing::warn!(%error, "failed to backfill forum threads");
    }
}

async fn channels(
    ctx: &serenity::Context,
    data: &Data,
    staff: &mut StaffLookup,
) -> anyhow::Result<()> {
    for &channel_id in &data.settings.record_channel_ids {
        let kind =
            match record::cached_channel(ctx, data.settings.guild_id, channel_id).map(|c| c.kind) {
                Some(kind) => kind,
                None => match channel_id.to_channel(ctx).await?.guild() {
                    Some(channel) => channel.kind,
                    None => continue,
                },
            };
        if kind == ChannelType::Forum {
            continue;
        }
        let Some(last) = db::last_message_id(&data.pool, snowflake(channel_id)).await? else {
            continue;
        };
        let count = fetch_and_record(
            ctx,
            data,
            staff,
            channel_id,
            Some(MessageId::new(last.cast_unsigned())),
            Scope::Channel,
        )
        .await?;
        if count > 0 {
            tracing::info!(
                channel_id = channel_id.get(),
                count,
                "backfilled channel messages"
            );
        }
    }
    Ok(())
}

async fn threads(
    ctx: &serenity::Context,
    data: &Data,
    staff: &mut StaffLookup,
) -> anyhow::Result<()> {
    let active = data.settings.guild_id.get_active_threads(ctx).await?;
    for thread in active.threads {
        let Some(input) = record::ticket_thread(ctx, data, &thread).await else {
            continue;
        };
        db::upsert_thread(&data.pool, &input).await?;
        let last = db::last_message_id(&data.pool, input.thread_id).await?;
        let count = fetch_and_record(
            ctx,
            data,
            staff,
            thread.id,
            last.map(|id| MessageId::new(id.cast_unsigned())),
            Scope::Thread,
        )
        .await?;
        if count > 0 {
            tracing::info!(
                thread_id = thread.id.get(),
                count,
                "backfilled thread messages"
            );
        }
    }
    Ok(())
}

async fn fetch_and_record(
    ctx: &serenity::Context,
    data: &Data,
    staff: &mut StaffLookup,
    channel_id: serenity::ChannelId,
    after: Option<MessageId>,
    scope: Scope,
) -> anyhow::Result<usize> {
    let mut builder = GetMessages::new().limit(LIMIT);
    if let Some(after) = after {
        builder = builder.after(after);
    }
    let mut messages = channel_id.messages(ctx, builder).await?;
    // 状態の遷移を発言順に進めるため、古い順に並べ直す
    messages.sort_by_key(|m| m.id);
    let mut count = 0;
    for message in messages.iter().filter(|m| record::is_recordable(m)) {
        let is_staff = staff.is_staff(ctx, data, message.author.id).await;
        record::record(ctx, data, message, scope, is_staff).await?;
        count += 1;
    }
    Ok(count)
}

/// HTTP で取ったメッセージにはメンバー情報が付かないので、送信者のロールを引いて覚えておく
#[derive(Default)]
struct StaffLookup(HashMap<UserId, bool>);

impl StaffLookup {
    async fn is_staff(&mut self, ctx: &serenity::Context, data: &Data, user_id: UserId) -> bool {
        if let Some(&is_staff) = self.0.get(&user_id) {
            return is_staff;
        }
        // サーバーを抜けた人はスタッフではない扱いにする
        let is_staff = match data.settings.guild_id.member(ctx, user_id).await {
            Ok(member) => data.is_staff(&member.roles),
            Err(error) => {
                tracing::debug!(%error, user_id = user_id.get(), "failed to fetch member");
                false
            }
        };
        self.0.insert(user_id, is_staff);
        is_staff
    }
}
