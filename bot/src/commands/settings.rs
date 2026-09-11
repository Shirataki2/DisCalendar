use poise::serenity_prelude::{self as serenity, ChannelId, ChannelType, Permissions};

use crate::{
    checks,
    data::Context,
    error::BotError,
    models::{event_settings, guild_config},
};

/// 現在の通知設定とBotの投稿権限を確認します（本人にだけ表示）
#[poise::command(
    slash_command,
    guild_only,
    ephemeral,
    check = "crate::checks::require_manage_permissions"
)]
pub async fn settings(ctx: Context<'_>) -> Result<(), BotError> {
    let Some(guild_id) = ctx.guild_id() else {
        return Err(BotError::user("このコマンドはサーバー内でのみ実行できます"));
    };
    ctx.defer_ephemeral().await?;
    let id = guild_id.to_string();
    let (destination, config) = tokio::join!(
        event_settings::get(&ctx.data().pool, &id),
        guild_config::get(&ctx.data().pool, &id)
    );
    let diagnosis = match &destination {
        Ok(None) => Diagnosis::Unset,
        Ok(Some(saved)) => match saved.channel_id.parse::<u64>() {
            Ok(id) if id != 0 => diagnose(ctx, ChannelId::new(id)).await,
            _ => Diagnosis::Invalid,
        },
        Err(_) => Diagnosis::Failed,
    };
    if let Err(error) = &destination {
        tracing::warn!(%error, "failed to read notification destination");
    }
    if let Err(error) = &config {
        tracing::warn!(%error, "failed to read notification configuration");
    }
    ctx.send(
        poise::CreateReply::default()
            .content(render(
                destination.as_ref().ok().and_then(|v| v.as_ref()),
                config.as_ref().ok(),
                diagnosis,
            ))
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true),
    )
    .await?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Diagnosis {
    Unset,
    Invalid,
    Deleted,
    Inaccessible,
    Failed,
    Checked(checks::ChannelPermissions),
}

fn channel_error(code: Option<isize>) -> Diagnosis {
    match code {
        Some(10003) => Diagnosis::Deleted,
        Some(50001 | 50013) => Diagnosis::Inaccessible,
        _ => Diagnosis::Failed,
    }
}

async fn diagnose(ctx: Context<'_>, channel_id: ChannelId) -> Diagnosis {
    // 削除・権限変更直後も診断できるよう、キャッシュを使わず現在の情報を取得する。
    let channel = match ctx.http().get_channel(channel_id).await {
        Ok(serenity::Channel::Guild(channel)) if Some(channel.guild_id) == ctx.guild_id() => {
            channel
        }
        Ok(_) => return Diagnosis::Invalid,
        Err(error) => {
            let code = match &error {
                serenity::Error::Http(serenity::HttpError::UnsuccessfulRequest(response)) => {
                    Some(response.error.code)
                }
                _ => None,
            };
            tracing::warn!(%error, "failed to fetch notification channel for diagnosis");
            return channel_error(code);
        }
    };
    let is_thread = matches!(
        channel.kind,
        ChannelType::PublicThread | ChannelType::PrivateThread | ChannelType::NewsThread
    );
    let guild_id = channel.guild_id;
    let bot_id = ctx.cache().current_user().id;
    let result: Result<_, serenity::Error> = async {
        let target = if is_thread {
            let Some(parent_id) = channel.parent_id else {
                return Ok(None);
            };
            let serenity::Channel::Guild(parent) = ctx.http().get_channel(parent_id).await? else {
                return Ok(None);
            };
            if parent.guild_id != guild_id {
                return Ok(None);
            }
            parent
        } else {
            channel
        };
        let guild = ctx.http().get_guild(guild_id).await?;
        let member = ctx.http().get_member(guild_id, bot_id).await?;
        Ok(Some(checks::ChannelPermissions {
            permissions: guild.user_permissions_in(&target, &member),
            is_thread,
        }))
    }
    .await;
    match result {
        Ok(Some(permissions)) => Diagnosis::Checked(permissions),
        Ok(None) => Diagnosis::Failed,
        Err(error) => {
            // 親チャンネルやメンバーの取得失敗を、通知先の削除と取り違えない。
            tracing::warn!(%error, "failed to fetch notification permissions for diagnosis");
            Diagnosis::Failed
        }
    }
}

fn render(
    destination: Option<&event_settings::EventSettings>,
    config: Option<&guild_config::GuildConfig>,
    diagnosis: Diagnosis,
) -> String {
    let channel = match destination {
        Some(saved) => match saved.channel_id.parse::<u64>() {
            Ok(id) if id != 0 => format!("<#{id}>（ID: {id}）"),
            _ => "保存されたチャンネル ID が不正です".into(),
        },
        None if diagnosis == Diagnosis::Unset => "未設定".into(),
        None => "取得失敗（未設定かどうかも確認できません）".into(),
    };
    let settings = match config {
        Some(config) => format!(
            "開始時刻の通知: {}\n既定の事前通知: {}",
            if config.notify_at_start { "有効" } else { "無効" },
            if config.default_notifications.is_empty() {
                "なし".into()
            } else {
                config.default_notifications.iter().map(ToString::to_string).collect::<Vec<_>>().join("、")
            }
        ),
        None => "開始時刻の通知: 取得失敗\n既定の事前通知: 取得失敗\n時間をおいて `/settings` を再実行してください。".into(),
    };
    let status = match diagnosis {
        Diagnosis::Unset => "通知先が未設定です。`/init` または Web のサーバー設定で通知先を設定してください。".into(),
        Diagnosis::Invalid => "保存された通知先をこのサーバーのチャンネルとして確認できません。`/init` または Web のサーバー設定で再設定してください。".into(),
        Diagnosis::Deleted => "Discord が「不明なチャンネル」を返しました。通知先は削除された可能性があります。`/init` または Web のサーバー設定で通知先を再設定してください。".into(),
        Diagnosis::Inaccessible => "Bot から通知先を参照できません。チャンネルの存在と Bot のロール・チャンネルの権限設定を確認し、必要なら `/init` で通知先を再設定してください。投稿権限は確認できていません。".into(),
        Diagnosis::Failed => "取得失敗のため投稿権限を確認できません。時間をおいて `/settings` を再実行してください。".into(),
        Diagnosis::Checked(bot) => {
            let required = checks::notification_permissions(bot.is_thread);
            let missing = required - bot.permissions;
            let details = [Permissions::VIEW_CHANNEL, if bot.is_thread { Permissions::SEND_MESSAGES_IN_THREADS } else { Permissions::SEND_MESSAGES }, Permissions::EMBED_LINKS]
                .into_iter().map(|permission| format!("{}: {}", checks::describe_permissions(permission), if bot.permissions.contains(permission) { "あり" } else { "なし" })).collect::<Vec<_>>().join("\n");
            let result = if missing.is_empty() {
                "投稿に必要な権限は揃っています。".into()
            } else {
                format!("不足する権限: {}。Bot のロールまたはチャンネルの権限設定を変更してください。", checks::describe_permissions(missing))
            };
            format!("{details}\n{result}")
        }
    };
    format!(
        "通知設定の確認\n通知先: {channel}\n{settings}\n\n{status}\n\n既定の事前通知は Web で予定を新規作成するときの初期値です。既存の予定や Bot の `/create`・`/quick` には自動適用されません。\nこの診断は設定・権限を変更せず、テスト投稿もしません。権限の確認結果は通知配信全体の正常性を保証しません。予定ごとの通知設定や Bot の稼働状況、スレッドの参加・アーカイブ状態なども確認してください。"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved() -> event_settings::EventSettings {
        event_settings::EventSettings {
            guild_id: "123".into(),
            channel_id: "123456789012345678".into(),
        }
    }

    #[test]
    fn unset_shows_defaults_and_setup_action() {
        let text = render(
            None,
            Some(&guild_config::GuildConfig::default()),
            Diagnosis::Unset,
        );
        assert!(text.contains("通知先: 未設定"));
        assert!(text.contains("開始時刻の通知: 有効"));
        assert!(text.contains("1日前、1時間前"));
        assert!(text.contains("`/init`"));
    }

    #[test]
    fn missing_permissions_and_saved_values_are_shown() {
        let config = guild_config::GuildConfig {
            notify_at_start: false,
            default_notifications: vec![],
            ..Default::default()
        };
        let text = render(
            Some(&saved()),
            Some(&config),
            Diagnosis::Checked(checks::ChannelPermissions {
                permissions: Permissions::VIEW_CHANNEL,
                is_thread: false,
            }),
        );
        assert!(text.contains("123456789012345678"));
        assert!(text.contains("開始時刻の通知: 無効"));
        assert!(text.contains("既定の事前通知: なし"));
        assert!(text.contains("不足する権限: 「メッセージを送信」「埋め込みリンク」"));
        assert!(text.contains("権限設定を変更"));
        assert!(!text.contains("権限は揃っています"));
    }

    #[test]
    fn permissions_do_not_guarantee_delivery_and_threads_use_thread_permission() {
        for is_thread in [false, true] {
            let text = render(
                Some(&saved()),
                Some(&Default::default()),
                Diagnosis::Checked(checks::ChannelPermissions {
                    permissions: checks::notification_permissions(is_thread),
                    is_thread,
                }),
            );
            assert!(text.contains("投稿に必要な権限は揃っています"));
            assert!(text.contains("通知配信全体の正常性を保証しません"));
            assert!(text.contains("テスト投稿もしません"));
            if is_thread {
                assert!(text.contains("「スレッドでメッセージを送信」: あり"));
            }
        }
    }

    #[test]
    fn failures_are_distinguished_and_never_report_success() {
        for (diagnosis, expected) in [
            (Diagnosis::Deleted, "削除された可能性"),
            (Diagnosis::Inaccessible, "参照できません"),
            (Diagnosis::Failed, "取得失敗"),
            (Diagnosis::Invalid, "確認できません"),
        ] {
            let text = render(Some(&saved()), None, diagnosis);
            assert!(text.contains(expected));
            assert!(text.contains("開始時刻の通知: 取得失敗"));
            assert!(!text.contains("権限は揃っています"));
        }
        assert!(render(None, None, Diagnosis::Failed).contains("未設定かどうかも確認できません"));
        assert_eq!(channel_error(Some(10003)), Diagnosis::Deleted);
        assert_eq!(channel_error(Some(50001)), Diagnosis::Inaccessible);
        assert_eq!(channel_error(Some(50013)), Diagnosis::Inaccessible);
        assert_eq!(channel_error(Some(0)), Diagnosis::Failed);
        assert_eq!(channel_error(None), Diagnosis::Failed);
    }

    #[test]
    fn command_is_private_guild_only_and_has_permission_check() {
        let command = settings();
        assert!(command.guild_only);
        assert!(command.ephemeral);
        assert_eq!(command.checks.len(), 1);
    }
}
