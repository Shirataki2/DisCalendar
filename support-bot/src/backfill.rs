//! 起動時の取りこぼし補完。Bot が止まっていた間の発言を、記録済みの最後のメッセージ以降から取り直す
//! (取りこぼすと「利用者が書いたのに未返信の通知が出ない」状態になるため)。
//!
//! - フォーラム: アクティブな投稿ごとに、記録済みの続きから最大 100 件 (未登録の投稿は直近 100 件)。
//!   停止中に付け外しされた「解決済み」タグも状態に反映する
//! - テキストチャンネル: 記録済みの続きから最大 100 件。一度も記録していないチャンネルは、導入前の
//!   発言まで取り込まないよう補完しない
//!
//! 起点は Gateway に繋ぐ前に DB から読んだ値 (`Data::backfill_from`) を使う。補完と並行して届く新着を
//! 記録すると DB の最大 ID が進み、それを起点にすると停止中の発言を飛ばしてしまうため。
//! 100 件を超えて取りこぼした分は諦める (長時間止めたときの負荷と、Discord の rate limit を避けるため)。

use std::collections::HashMap;

use poise::serenity_prelude::{self as serenity, ChannelType, GetMessages, MessageId, UserId};

use crate::{
    data::Data,
    db::{self, ThreadInput},
    record::{self, Scope, snowflake},
    ticket::{self, Status},
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
        let Some(&last) = data.backfill_from.get(&snowflake(channel_id)) else {
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
        reconcile_resolved_tag(data, &input).await?;
        let last = data.backfill_from.get(&input.thread_id).copied();
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

/// 停止中に「解決済み」タグが付け外しされていたら、DB の状態を合わせる (ThreadUpdate を受け取れなかった分)。
/// 発言の補完より先に行う。停止中の利用者の発言はそのあとの補完で再オープンにするので、
/// タグと発言の前後が分からなくても、対応漏れ (解決済みのまま利用者が待っている) にはならない
async fn reconcile_resolved_tag(data: &Data, input: &ThreadInput) -> anyhow::Result<()> {
    let has_tag = input.tags.iter().any(|name| name == ticket::RESOLVED_TAG);
    let target = match db::thread_status(&data.pool, input.thread_id).await? {
        Some(status) if has_tag && status != Status::Resolved => Some(Status::Resolved),
        // タグが 5 個付いていて Bot が「解決済み」を足せなかったスレッドは、外されたとはみなさない
        Some(Status::Resolved) if !has_tag && input.tags.len() < 5 => None,
        _ => return Ok(()),
    };
    if let Some(transition) = db::set_status(&data.pool, input.thread_id, target).await? {
        tracing::info!(
            thread_id = input.thread_id,
            before = transition.before.as_db(),
            after = transition.after.as_db(),
            "ticket status reconciled with forum tags"
        );
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
