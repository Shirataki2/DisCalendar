use poise::serenity_prelude::{CreateEmbed, CreateEmbedFooter, Timestamp};

use crate::{data::Context, error::BotError};

/// このBotの使い方を表示します
#[poise::command(slash_command)]
pub async fn help(ctx: Context<'_>) -> Result<(), BotError> {
    // 本文は help.txt。`{dummy}` は Discord の埋め込みで行頭の字下げに使う全角スペース
    let description = format!(
        include_str!("help.txt"),
        dummy = "　",
        invite = ctx.data().invite_url
    );
    // CurrentUserRef はキャッシュのロックなので、URL だけ取り出してすぐ手放す
    let thumbnail = ctx.cache().current_user().face();
    let embed = CreateEmbed::new()
        .title("DisCalendar - Help")
        .description(description)
        .colour(0x0000dd)
        .timestamp(Timestamp::now())
        .footer(CreateEmbedFooter::new(format!("v{}", crate::VERSION)))
        .thumbnail(thumbnail);
    ctx.send(poise::CreateReply::default().embed(embed).ephemeral(true))
        .await?;
    Ok(())
}
