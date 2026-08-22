use poise::serenity_prelude::ChannelId;

use crate::{checks, data::Context, error::BotError, models::event_settings};

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

    // DB を書く前に「処理中」を返し、3 秒の初回応答期限を過ぎて interaction が失敗扱いになるのを防ぐ
    // (権限チェックは check 関数で応答前に済んでいて、権限なしの返信は本人にだけ見える)
    ctx.defer().await?;

    // Bot が投稿できないチャンネル (権限上書きで閲覧・送信が禁止されているなど) を保存しても通知は届かないので、
    // 保存前に Bot 自身の権限を確認する。権限が分からないときは旧 Bot と同じく保存する (ログに残す)
    match checks::bot_permissions_in(ctx, channel_id).await? {
        Some(bot) => {
            let missing = checks::notification_permissions(bot.is_thread) - bot.permissions;
            if !missing.is_empty() {
                return Err(BotError::user(format!(
                    "<#{channel_id}> で Bot に {} の権限がないため、通知を投稿できません。\
                     Bot のロールかチャンネルの権限設定を見直してから、もう一度実行してください",
                    checks::describe_permissions(missing)
                )));
            }
        }
        None => tracing::warn!(
            guild_id = guild_id.get(),
            channel_id = channel_id.get(),
            "could not determine the bot's permissions in the channel; saving anyway"
        ),
    }

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
