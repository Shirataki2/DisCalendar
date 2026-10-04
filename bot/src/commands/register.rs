use crate::{
    data::{Context, Data},
    error::BotError,
    i18n::{Locale, user_locale},
};
use poise::serenity_prelude as serenity;

/// poise がコンテキストメニューで省略する翻訳メタデータも付ける。
pub fn definitions(commands: &[poise::Command<Data, BotError>]) -> Vec<serenity::CreateCommand> {
    commands
        .iter()
        .flat_map(|command| {
            let slash = command.create_as_slash_command();
            let menu = command.create_as_context_menu_command().map(|mut menu| {
                for (locale, name) in &command.name_localizations {
                    menu = menu.name_localized(locale, name);
                }
                menu
            });
            slash.into_iter().chain(menu)
        })
        .collect()
}

/// スラッシュコマンドを Discord に登録・削除します (Bot のオーナー専用)。
#[poise::command(prefix_command, owners_only, hide_in_help)]
pub async fn register(ctx: Context<'_>) -> Result<(), BotError> {
    let locale = user_locale(ctx);
    let choices = [
        (
            "register.guild",
            "このサーバーに登録",
            "Register in guild",
            false,
        ),
        (
            "unregister.guild",
            "このサーバーから削除",
            "Delete in guild",
            true,
        ),
        (
            "register.global",
            "全サーバーに登録",
            "Register globally",
            false,
        ),
        (
            "unregister.global",
            "全サーバーから削除",
            "Unregister globally",
            true,
        ),
    ];
    let buttons = choices
        .iter()
        .map(|(id, ja, en, delete)| {
            serenity::CreateButton::new(format!("{}:{id}", ctx.id()))
                .label(locale.text(ja, en))
                .style(if *delete {
                    serenity::ButtonStyle::Danger
                } else {
                    serenity::ButtonStyle::Primary
                })
        })
        .collect();
    let reply = ctx
        .send(
            poise::CreateReply::default()
                .content(locale.text(
                    "コマンドの登録・削除を選択してください",
                    "Choose how to register or delete the commands.",
                ))
                .components(vec![serenity::CreateActionRow::Buttons(buttons)]),
        )
        .await?;
    let invocation_id = ctx.id();
    let interaction = serenity::ComponentInteractionCollector::new(ctx.serenity_context())
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .message_id(reply.message().await?.id)
        .filter(move |press| {
            choices
                .iter()
                .any(|(id, _, _, _)| press.data.custom_id == format!("{invocation_id}:{id}"))
        })
        .timeout(std::time::Duration::from_secs(120))
        .await;
    let Some(interaction) = interaction else {
        reply
            .edit(
                ctx,
                poise::CreateReply::default()
                    .components(vec![])
                    .content(locale.text(
                        "時間切れです。コマンドを再実行してください",
                        "Timed out. Please run the command again.",
                    )),
            )
            .await?;
        return Ok(());
    };
    let locale = Locale::resolve(&interaction.locale);
    interaction
        .create_response(ctx.http(), serenity::CreateInteractionResponse::Acknowledge)
        .await?;
    reply
        .edit(
            ctx,
            poise::CreateReply::default()
                .components(vec![])
                .content(locale.text("処理しています…", "Processing…")),
        )
        .await?;
    // ボタンを押せるのはコマンドを実行したオーナーだけ。
    let action = interaction
        .data
        .custom_id
        .split_once(':')
        .map(|(_, action)| action)
        .unwrap_or_default();
    let commands = if action.starts_with("register.") {
        definitions(&ctx.framework().options().commands)
    } else {
        vec![]
    };
    if action.ends_with("global") {
        serenity::Command::set_global_commands(ctx.http(), commands).await?;
    } else {
        let guild = ctx.guild_id().ok_or_else(|| {
            crate::user_error!(
                "サーバー内で実行してください",
                "Run this command in a server."
            )
        })?;
        guild.set_commands(ctx.http(), commands).await?;
    }
    reply
        .edit(
            ctx,
            poise::CreateReply::default()
                .components(vec![])
                .content(locale.text(
                    "コマンドの登録・削除が完了しました",
                    "Command registration or deletion completed.",
                )),
        )
        .await?;
    Ok(())
}
