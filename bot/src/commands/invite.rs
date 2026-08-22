use poise::serenity_prelude::{ApplicationId, Permissions};

use crate::{data::Context, error::BotError};

/// Botを他のサーバーに招待するためのURLを表示します
#[poise::command(slash_command)]
pub async fn invite(ctx: Context<'_>) -> Result<(), BotError> {
    ctx.say(ctx.data().invite_url.clone()).await?;
    Ok(())
}

/// 招待時に求める権限 (web の `DEFAULT_BOT_PERMISSIONS` と同じ):
/// チャンネルを見る / メッセージを送信 / 埋め込みリンク / メッセージ履歴を読む / アプリコマンドを使う
pub fn required_bot_permissions() -> Permissions {
    Permissions::VIEW_CHANNEL
        | Permissions::SEND_MESSAGES
        | Permissions::EMBED_LINKS
        | Permissions::READ_MESSAGE_HISTORY
        | Permissions::USE_APPLICATION_COMMANDS
}

/// 招待 URL の既定値 (`DISCORD_BOT_INVITE_URL` が未設定のとき)。
/// web の `botInviteUrl` と同じスコープ・権限で、アプリケーション ID は起動時に Discord から受け取る
pub fn default_invite_url(application_id: ApplicationId) -> String {
    format!(
        "https://discord.com/oauth2/authorize?client_id={application_id}&scope=bot%20applications.commands&permissions={}",
        required_bot_permissions().bits()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_match_the_web_invite_url() {
        // web/src/lib/discord.ts の DEFAULT_BOT_PERMISSIONS
        assert_eq!(required_bot_permissions().bits(), 2_147_568_640);
    }

    #[test]
    fn builds_invite_url_from_application_id() {
        assert_eq!(
            default_invite_url(ApplicationId::new(771795045543313409)),
            "https://discord.com/oauth2/authorize?client_id=771795045543313409&scope=bot%20applications.commands&permissions=2147568640"
        );
    }
}
