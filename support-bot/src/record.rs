//! Discord のメッセージ・スレッドを記録の形に直し、記録対象かどうかを判定する。
//!
//! 記録するのは許可リスト (`SUPPORT_RECORD_CHANNEL_IDS`) のチャンネルだけ。
//! - テキストチャンネル: そのチャンネルへの発言 (配下のスレッドは対象外)
//! - フォーラム: 配下の投稿 (スレッド) とその発言。投稿 1 件をチケット 1 件として扱う

use chrono::{DateTime, Utc};
use poise::serenity_prelude::{self as serenity, ChannelType, GuildChannel, Message, MessageType};

use crate::{
    data::Data,
    db::{self, AttachmentRecord, MessageInput, ThreadInput},
    ticket,
};

/// メッセージの置き場所
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// 許可リストのテキストチャンネル
    Channel,
    /// 許可リストのフォーラムの投稿 (チケット)
    Thread,
}

pub fn to_utc(timestamp: serenity::Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(timestamp.unix_timestamp(), 0).unwrap_or_default()
}

pub fn snowflake(id: impl Into<u64>) -> i64 {
    // Discord の ID は 2^63 未満なので i64 に収まる
    i64::try_from(id.into()).expect("Discord snowflakes fit in i64")
}

pub fn attachments(message_attachments: &[serenity::Attachment]) -> Vec<AttachmentRecord> {
    message_attachments
        .iter()
        .map(|a| AttachmentRecord {
            filename: a.filename.clone(),
            url: a.url.clone(),
            size: a.size,
            content_type: a.content_type.clone(),
        })
        .collect()
}

/// キャッシュにあるギルドのチャンネル (スレッドは含まない)
pub fn cached_channel(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    channel_id: serenity::ChannelId,
) -> Option<GuildChannel> {
    ctx.cache
        .guild(guild_id)
        .and_then(|guild| guild.channels.get(&channel_id).cloned())
}

/// 記録する種類のメッセージか (通常の発言と返信だけ。ピン留めやスレッド作成の通知などは除く)
pub fn is_recordable(message: &Message) -> bool {
    !message.author.bot
        && matches!(
            message.kind,
            MessageType::Regular | MessageType::InlineReply
        )
}

/// スレッドが記録対象のフォーラムの投稿なら、登録用の値を返す
pub async fn ticket_thread(
    ctx: &serenity::Context,
    data: &Data,
    thread: &GuildChannel,
) -> Option<ThreadInput> {
    if thread.guild_id != data.settings.guild_id {
        return None;
    }
    let parent_id = thread.parent_id?;
    if !data.is_record_channel(parent_id) {
        return None;
    }
    if !matches!(
        thread.kind,
        ChannelType::PublicThread | ChannelType::PrivateThread
    ) {
        return None;
    }
    // フォーラムかどうかはキャッシュの親チャンネルで見る (テキストチャンネル配下のスレッドは記録しない)
    let parent_kind = match cached_channel(ctx, thread.guild_id, parent_id).map(|c| c.kind) {
        Some(kind) => kind,
        None => parent_id.to_channel(ctx).await.ok()?.guild()?.kind,
    };
    if parent_kind != ChannelType::Forum {
        return None;
    }
    let available = ticket::forum_tags(ctx, thread.guild_id, parent_id)
        .await
        .unwrap_or_default();
    Some(ThreadInput {
        thread_id: snowflake(thread.id),
        parent_channel_id: snowflake(parent_id),
        author_id: thread.owner_id.map(snowflake),
        title: thread.name.clone(),
        tags: ticket::tag_names(&available, &thread.applied_tags),
        archived: thread
            .thread_metadata
            .is_some_and(|metadata| metadata.archived),
    })
}

/// メッセージの置き場所を判定する。未登録のスレッドなら、ここで登録する (Bot の停止中に作られた投稿など)
pub async fn scope_of(
    ctx: &serenity::Context,
    data: &Data,
    channel_id: serenity::ChannelId,
) -> Result<Option<Scope>, crate::error::BotError> {
    if data.is_record_channel(channel_id) {
        return Ok(Some(Scope::Channel));
    }
    if db::thread_exists(&data.pool, snowflake(channel_id)).await? {
        return Ok(Some(Scope::Thread));
    }
    let channel = match channel_id.to_channel(ctx).await {
        Ok(channel) => channel,
        Err(error) => {
            tracing::debug!(%error, channel_id = channel_id.get(), "failed to fetch channel");
            return Ok(None);
        }
    };
    let Some(thread) = channel.guild() else {
        return Ok(None);
    };
    match ticket_thread(ctx, data, &thread).await {
        Some(input) => {
            db::upsert_thread(&data.pool, &input).await?;
            Ok(Some(Scope::Thread))
        }
        None => Ok(None),
    }
}

/// 記録用の値を作る。スタッフかどうかは `is_staff` で渡す (Gateway の発言なら message.member、補完なら HTTP で取る)
pub fn message_input(message: &Message, scope: Scope, is_staff: bool) -> MessageInput {
    MessageInput {
        message_id: snowflake(message.id),
        channel_id: snowflake(message.channel_id),
        thread_id: (scope == Scope::Thread).then(|| snowflake(message.channel_id)),
        author_id: snowflake(message.author.id),
        author_is_staff: is_staff,
        content: message.content.clone(),
        attachments: attachments(&message.attachments),
        created_at: to_utc(message.timestamp),
        edited_at: message.edited_timestamp.map(to_utc),
    }
}

/// 記録して、チケットの状態が変わったらタグを合わせる
pub async fn record(
    ctx: &serenity::Context,
    data: &Data,
    message: &Message,
    scope: Scope,
    is_staff: bool,
) -> Result<(), crate::error::BotError> {
    let input = message_input(message, scope, is_staff);
    if let Some(transition) = db::record_message(&data.pool, &input).await?
        && transition.changed()
    {
        tracing::info!(
            thread_id = message.channel_id.get(),
            before = transition.before.as_db(),
            after = transition.after.as_db(),
            "ticket status changed by a message"
        );
        if transition.resolved_changed() {
            ticket::sync_tags(ctx, message.channel_id, transition.after).await;
        }
    }
    Ok(())
}
