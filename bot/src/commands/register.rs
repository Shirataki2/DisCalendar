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

/// 既存のグローバルコマンドが Activity の Entry Point (type 4) なら、一括上書きに含め直す定義を返す。
/// Entry Point は Activity を有効にすると Discord が作るもので、Bot 側では定義しない
fn keep_entry_point(command: &serenity::Command) -> Option<serenity::CreateCommand> {
    if command.kind != serenity::CommandType::PrimaryEntryPoint {
        return None;
    }
    let mut create = serenity::CreateCommand::new(&command.name)
        .kind(serenity::CommandType::PrimaryEntryPoint)
        .description(&command.description)
        .nsfw(command.nsfw)
        .integration_types(command.integration_types.clone());
    if let Some(handler) = command.handler {
        create = create.handler(handler);
    }
    if let Some(contexts) = &command.contexts {
        create = create.contexts(contexts.clone());
    }
    for (locale, name) in command.name_localizations.iter().flatten() {
        create = create.name_localized(locale, name);
    }
    for (locale, description) in command.description_localizations.iter().flatten() {
        create = create.description_localized(locale, description);
    }
    Some(create)
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
        // Activity (#350) を有効にしたアプリには Discord が Entry Point コマンドを作り、
        // それを含まない一括上書きは拒否される (50240)。既存の Entry Point をそのまま残して上書きする
        let existing = serenity::Command::get_global_commands(ctx.http()).await?;
        let commands = commands
            .into_iter()
            .chain(existing.iter().filter_map(keep_entry_point))
            .collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn command(kind: u8) -> serenity::Command {
        serde_json::from_value(json!({
            "id": "1",
            "type": kind,
            "application_id": "2",
            "name": "launch",
            "name_localizations": { "ja": "起動" },
            "description": "Launch the activity",
            "default_member_permissions": null,
            "nsfw": false,
            "integration_types": [0, 1],
            "contexts": [0, 1, 2],
            "version": "3",
            "handler": 2
        }))
        .unwrap()
    }

    #[test]
    fn keeps_entry_point_as_is() {
        let kept = keep_entry_point(&command(4)).expect("Entry Point は残す");
        let value = serde_json::to_value(kept).unwrap();
        assert_eq!(value["type"], 4);
        assert_eq!(value["name"], "launch");
        assert_eq!(value["description"], "Launch the activity");
        assert_eq!(value["handler"], 2);
        assert_eq!(value["integration_types"], json!([0, 1]));
        assert_eq!(value["contexts"], json!([0, 1, 2]));
        assert_eq!(value["name_localizations"]["ja"], "起動");
    }

    #[test]
    fn ignores_other_commands() {
        assert!(keep_entry_point(&command(1)).is_none());
        assert!(keep_entry_point(&command(2)).is_none());
    }
}
