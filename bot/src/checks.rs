//! コマンドの権限チェック (旧 `utils/checks.rs` 相当)。
//!
//! 判定は api の `can_manage_server` と同じ「管理者 / サーバー管理 / メッセージの管理 / ロールの管理 のいずれか」で、
//! ギルドレベルの基本パーミッション (ロールの OR、オーナーは全権限) を見る。チャンネルごとの上書きは考慮しない
//! (web の restricted モードの判定と揃えるため)。

use poise::serenity_prelude::{self as serenity, Permissions};

use crate::{data::Context, error::BotError};

/// 権限が足りないときにユーザーへ返すメッセージ
pub const MANAGE_PERMISSIONS_REQUIRED: &str = "このコマンドを実行するには「管理者」「サーバー管理」「メッセージの管理」「ロールの管理」のいずれかの権限が必要です";

/// 旧実装・api と同じ「サーバー管理」判定。`administrator` を含むかどうかは serenity が各メソッドで考慮する
pub fn can_manage_server(permissions: Permissions) -> bool {
    permissions.administrator()
        || permissions.manage_guild()
        || permissions.manage_messages()
        || permissions.manage_roles()
}

/// コマンドを実行したユーザーのギルドレベルの権限。DM やメンバー情報が取れないときは `None`。
///
/// スラッシュコマンドでは interaction にメンバー情報 (ロール一覧) が付いてくるので、
/// キャッシュ上のギルド (ロール定義・オーナー) と突き合わせて計算する。
/// キャッシュにギルドがなければ HTTP で取り直す
pub async fn author_permissions(ctx: Context<'_>) -> Result<Option<Permissions>, BotError> {
    let Some(guild_id) = ctx.guild_id() else {
        return Ok(None);
    };
    let Some(member) = ctx.author_member().await else {
        return Ok(None);
    };
    // GuildRef はキャッシュのロック。await をまたがないよう、この式の中で計算して値だけ取り出す
    if let Some(permissions) = ctx.guild().map(|guild| guild.member_permissions(&member)) {
        return Ok(Some(permissions));
    }
    let guild: serenity::PartialGuild = guild_id.to_partial_guild(ctx.http()).await?;
    Ok(Some(guild.member_permissions(&member)))
}

/// コマンドを実行したユーザーが「サーバー管理」権限を持つか。権限が分からないときは安全側に倒して false
pub async fn author_can_manage_server(ctx: Context<'_>) -> Result<bool, BotError> {
    Ok(author_permissions(ctx)
        .await?
        .is_some_and(can_manage_server))
}

/// `#[poise::command(check = "...")]` 用。権限がなければ本人にだけ理由を返して false
pub async fn require_manage_permissions(ctx: Context<'_>) -> Result<bool, BotError> {
    if author_can_manage_server(ctx).await? {
        return Ok(true);
    }
    ctx.send(
        poise::CreateReply::default()
            .content(MANAGE_PERMISSIONS_REQUIRED)
            .ephemeral(true),
    )
    .await?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_of_the_four_manage_permissions_is_enough() {
        for p in [
            Permissions::ADMINISTRATOR,
            Permissions::MANAGE_GUILD,
            Permissions::MANAGE_MESSAGES,
            Permissions::MANAGE_ROLES,
            Permissions::MANAGE_GUILD | Permissions::SEND_MESSAGES,
        ] {
            assert!(can_manage_server(p), "{p:?}");
        }
    }

    #[test]
    fn other_permissions_are_not_enough() {
        for p in [
            Permissions::empty(),
            Permissions::SEND_MESSAGES | Permissions::VIEW_CHANNEL,
            Permissions::MANAGE_CHANNELS | Permissions::KICK_MEMBERS,
        ] {
            assert!(!can_manage_server(p), "{p:?}");
        }
    }
}
