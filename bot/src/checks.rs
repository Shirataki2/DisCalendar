//! コマンドの権限チェック (旧 `utils/checks.rs` 相当)。
//!
//! 判定は api の `can_manage_server` と同じ「管理者 / サーバー管理 / メッセージの管理 / ロールの管理 のいずれか」で、
//! ギルドレベルの基本パーミッション (ロールの OR、オーナーは全権限) を見る。チャンネルごとの上書きは考慮しない
//! (web の restricted モードの判定と揃えるため)。

use poise::serenity_prelude::{self as serenity, ChannelId, ChannelType, Permissions};

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

/// Bot 自身のあるチャンネルでの権限
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelPermissions {
    /// ロールとチャンネルの権限上書きを反映した権限 (管理者なら全権限)
    pub permissions: Permissions,
    /// スレッドかどうか (投稿に必要な権限が `SEND_MESSAGES_IN_THREADS` になる)
    pub is_thread: bool,
}

/// 予定の通知を投稿するのに必要な権限: チャンネルを見る / メッセージを送信 (スレッドではスレッドでメッセージを送信) / 埋め込みリンク
pub fn notification_permissions(is_thread: bool) -> Permissions {
    let send = if is_thread {
        Permissions::SEND_MESSAGES_IN_THREADS
    } else {
        Permissions::SEND_MESSAGES
    };
    Permissions::VIEW_CHANNEL | send | Permissions::EMBED_LINKS
}

/// 権限の日本語名を「」で囲んで並べる (通知に関係する権限だけ)。例: 「チャンネルを見る」「埋め込みリンク」
pub fn describe_permissions(permissions: Permissions) -> String {
    [
        (Permissions::VIEW_CHANNEL, "チャンネルを見る"),
        (Permissions::SEND_MESSAGES, "メッセージを送信"),
        (
            Permissions::SEND_MESSAGES_IN_THREADS,
            "スレッドでメッセージを送信",
        ),
        (Permissions::EMBED_LINKS, "埋め込みリンク"),
    ]
    .into_iter()
    .filter(|(permission, _)| permissions.contains(*permission))
    .map(|(_, name)| format!("「{name}」"))
    .collect()
}

/// Bot 自身の `channel_id` での権限 (ロール + チャンネルの権限上書き)。
/// スレッドなら親チャンネルの権限で計算する (スレッド自体には上書きがない)。
/// DM や、チャンネル・メンバー情報が取れないときは `None`
pub async fn bot_permissions_in(
    ctx: Context<'_>,
    channel_id: ChannelId,
) -> Result<Option<ChannelPermissions>, BotError> {
    let Some(guild_id) = ctx.guild_id() else {
        return Ok(None);
    };
    // CurrentUserRef / GuildRef はキャッシュのロックなので、値を取り出してすぐ手放す
    let bot_id = ctx.cache().current_user().id;
    // 自分のメンバー情報 (ロール一覧)。GuildCreate に含まれるのでキャッシュにあることが多く、なければ HTTP で取る
    let member = match ctx
        .guild()
        .and_then(|guild| guild.members.get(&bot_id).cloned())
    {
        Some(member) => member,
        None => guild_id.member(ctx.serenity_context(), bot_id).await?,
    };
    // チャンネル (権限上書き付き) は GUILDS インテントでキャッシュに同期される。スレッドは `to_channel` で取る
    let serenity::Channel::Guild(channel) = channel_id.to_channel(ctx.serenity_context()).await?
    else {
        return Ok(None);
    };
    let is_thread = matches!(
        channel.kind,
        ChannelType::PublicThread | ChannelType::PrivateThread | ChannelType::NewsThread
    );
    let target = if is_thread {
        let Some(parent_id) = channel.parent_id else {
            return Ok(None);
        };
        let serenity::Channel::Guild(parent) = parent_id.to_channel(ctx.serenity_context()).await?
        else {
            return Ok(None);
        };
        parent
    } else {
        channel
    };
    let permissions = match ctx
        .guild()
        .map(|guild| guild.user_permissions_in(&target, &member))
    {
        Some(permissions) => permissions,
        None => {
            let guild = guild_id.to_partial_guild(ctx.serenity_context()).await?;
            guild.user_permissions_in(&target, &member)
        }
    };
    Ok(Some(ChannelPermissions {
        permissions,
        is_thread,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_permissions_depend_on_thread() {
        assert_eq!(
            notification_permissions(false),
            Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::EMBED_LINKS
        );
        assert_eq!(
            notification_permissions(true),
            Permissions::VIEW_CHANNEL
                | Permissions::SEND_MESSAGES_IN_THREADS
                | Permissions::EMBED_LINKS
        );
        // 管理者は全権限なので何も足りない
        assert!((notification_permissions(false) - Permissions::all()).is_empty());
        // 閲覧だけだと送信と埋め込みが足りない
        let missing = notification_permissions(false) - Permissions::VIEW_CHANNEL;
        assert_eq!(
            missing,
            Permissions::SEND_MESSAGES | Permissions::EMBED_LINKS
        );
    }

    #[test]
    fn describes_permissions_in_japanese() {
        assert_eq!(
            describe_permissions(Permissions::SEND_MESSAGES | Permissions::EMBED_LINKS),
            "「メッセージを送信」「埋め込みリンク」"
        );
        assert_eq!(
            describe_permissions(Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES_IN_THREADS),
            "「チャンネルを見る」「スレッドでメッセージを送信」"
        );
        // 通知に関係ない権限は出さない
        assert_eq!(describe_permissions(Permissions::MANAGE_GUILD), "");
    }

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
