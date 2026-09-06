//! スラッシュコマンド (旧 `commands/` 相当)。
//!
//! ユーザー向けのコマンド (help / create / list / init / invite) はスラッシュコマンドのみ。
//! 旧版の `cal ...` プレフィックスは MESSAGE_CONTENT 特権インテントがないとメッセージ本文が届かないので設けない。
//! `register` (スラッシュコマンドの登録、オーナー専用) だけはメンション (`@DisCalendar register`) で呼ぶ
//! プレフィックスコマンド (Bot 宛てのメンションがあるメッセージは本文が届く)

pub mod create;
pub mod help;
pub mod init;
pub mod invite;
pub mod list;
pub mod register;

pub use crate::datetime::format_datetime;

use crate::{
    data::{Context, Data},
    error::BotError,
};

/// フレームワークに登録する全コマンド
pub fn all() -> Vec<poise::Command<Data, BotError>> {
    vec![
        help::help(),
        create::create(),
        list::list(),
        init::init(),
        invite::invite(),
        register::register(),
    ]
}

/// `pre_command`: 実行されたコマンドをログに出す (旧 `event.rs::pre_command`)。引数の値は出さない
pub async fn log_invocation(ctx: Context<'_>) {
    tracing::info!(
        command = %ctx.command().qualified_name,
        user_id = ctx.author().id.get(),
        guild_id = ?ctx.guild_id().map(|g| g.get()),
        channel_id = ctx.channel_id().get(),
        "command invoked"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    /// Discord がスラッシュコマンドの登録時に要求する制約。破っていると `register` が 400 で失敗する
    #[test]
    fn slash_commands_satisfy_discord_limits() {
        let commands = all();
        let slash: Vec<_> = commands
            .iter()
            .filter(|c| c.slash_action.is_some())
            .collect();
        assert_eq!(
            slash.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["help", "create", "list", "init", "invite"]
        );
        for command in &slash {
            let description = command.description.as_deref().unwrap_or("");
            assert!(
                !description.is_empty() && description.chars().count() <= 100,
                "{}: {description}",
                command.name
            );
            assert!(command.parameters.len() <= 25, "{}", command.name);
            // 必須の引数は任意の引数より前に並べる
            let first_optional = command
                .parameters
                .iter()
                .position(|p| !p.required)
                .unwrap_or(command.parameters.len());
            assert!(
                command.parameters[first_optional..]
                    .iter()
                    .all(|p| !p.required),
                "{}: required parameter after an optional one",
                command.name
            );
            for parameter in &command.parameters {
                assert!(
                    parameter
                        .name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                    "{}.{}",
                    command.name,
                    parameter.name
                );
                let description = parameter.description.as_deref().unwrap_or("");
                assert!(
                    !description.is_empty() && description.chars().count() <= 100,
                    "{}.{}: {description}",
                    command.name,
                    parameter.name
                );
                assert!(
                    parameter.choices.len() <= 25,
                    "{}.{}",
                    command.name,
                    parameter.name
                );
            }
        }
        // Discord に送る定義が組み立てられる (slash の 5 つ)
        assert_eq!(
            poise::builtins::create_application_commands(&commands).len(),
            5
        );
    }

    #[test]
    fn register_is_an_owner_only_prefix_command() {
        let commands = all();
        let register = commands.iter().find(|c| c.name == "register").unwrap();
        // スラッシュコマンドの登録前に呼べる必要があるので、スラッシュコマンドにはしない
        assert!(register.slash_action.is_none());
        assert!(register.prefix_action.is_some());
        assert!(register.owners_only);
        assert!(register.hide_in_help);
    }

    #[test]
    fn create_keeps_the_legacy_parameter_set() {
        let commands = all();
        let create = commands.iter().find(|c| c.name == "create").unwrap();
        assert!(create.guild_only);
        let names: Vec<_> = create.parameters.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "name",
                "start_year",
                "start_month",
                "start_day",
                "start_hour",
                "start_minute",
                "end_year",
                "end_month",
                "end_day",
                "end_hour",
                "end_minute",
                "description",
                "is_all_day",
                "color",
                "notify_1",
                "notify_2",
                "notify_3",
                "notify_4",
            ]
        );
        for guild_only in ["list", "init"] {
            assert!(
                commands
                    .iter()
                    .find(|c| c.name == guild_only)
                    .unwrap()
                    .guild_only,
                "{guild_only}"
            );
        }
    }

    #[test]
    fn formats_datetime_for_discord_while_preserving_all_day_dates() {
        let dt: NaiveDateTime = "2026-08-23T09:05:00".parse().unwrap();
        assert_eq!(
            format_datetime(dt, false),
            "<t:1787443500:F> (<t:1787443500:R>)"
        );
        assert_eq!(format_datetime(dt, true), "2026/08/23");
    }
}
