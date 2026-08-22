use poise::serenity_prelude::ChannelId;

use crate::{data::Context, error::BotError, models::event_settings};

/// このBotの通知の送信先チャンネルを設定します
///
/// 予定の開始時刻 (と事前通知) にこのチャンネルへ投稿する。
/// 「管理者」「サーバー管理」「メッセージの管理」「ロールの管理」のいずれかの権限が必要
#[poise::command(
    slash_command,
    guild_only,
    check = "crate::checks::require_manage_permissions"
)]
pub async fn init(
    ctx: Context<'_>,
    #[description = "通知先のチャンネル (指定しない場合はこのコマンドを実行したチャンネル)"]
    #[channel_types("Text", "News")]
    channel: Option<ChannelId>,
) -> Result<(), BotError> {
    let Some(guild_id) = ctx.guild_id() else {
        return Err(BotError::user("このコマンドはサーバー内でのみ実行できます"));
    };
    let channel_id = channel.unwrap_or_else(|| ctx.channel_id());

    let previous = event_settings::set(
        &ctx.data().pool,
        &guild_id.to_string(),
        &channel_id.to_string(),
    )
    .await?;
    tracing::info!(
        guild_id = guild_id.get(),
        channel_id = channel_id.get(),
        user_id = ctx.author().id.get(),
        "notification channel set"
    );

    let message = match previous {
        Some(previous) => format!(
            "イベント通知先を変更しました\n通知先: <#{}> → <#{}>",
            previous.channel_id, channel_id
        ),
        None => format!("イベント通知を有効にしました\n通知先: <#{channel_id}>"),
    };
    ctx.say(message).await?;
    Ok(())
}
