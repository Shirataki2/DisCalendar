//! Bot がサーバーに参加したときの最初の案内 (#376)。
//!
//! 導入した人が次に何をすればよいか分かるように、新しく参加したとき (`GuildCreate` の `is_new`) に
//! 1 回だけ短い案内を送る。起動時の `GuildCreate` (参加済みのサーバー) では呼ばない。
//!
//! - 送り先: サーバーのシステムチャンネル。Bot が投稿できなければ、投稿できる最初のテキストチャンネル
//!   (チャンネル一覧の並び順)。どこにも投稿できなければ何もしない
//! - 言語: web の設定で保存したサーバー言語 (`guild_config.locale`)。保存していなければ Discord の
//!   サーバー言語 (`preferred_locale`)。未対応の言語は日本語 (docs/localization.md)
//! - 同じサーバーには [`guild_welcome::RESEND_AFTER_DAYS`] 日に 1 回まで (退出直後の再参加で何度も送らない)
//! - 失敗しても参加の処理 (DB 登録・運営ログへの通知) には影響させず、ログに残すだけにする

use poise::serenity_prelude::{self as serenity, ChannelId, ChannelType, GuildId, Permissions};

use crate::{
    data::Data,
    i18n::Locale,
    models::{guild_welcome, now_jst},
};

/// 案内を送るのに要る権限 (埋め込みとリンクボタン。リンクボタンに追加の権限は要らない)
fn required_permissions() -> Permissions {
    Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::EMBED_LINKS
}

/// 送り先を選ぶ。`channels` はテキストチャンネルを並び順に並べ、Bot の権限を添えたもの
pub fn pick_channel(
    system_channel: Option<ChannelId>,
    channels: &[(ChannelId, Permissions)],
) -> Option<ChannelId> {
    let can_send = |permissions: Permissions| permissions.contains(required_permissions());
    system_channel
        .filter(|system| {
            channels
                .iter()
                .any(|(id, permissions)| id == system && can_send(*permissions))
        })
        .or_else(|| {
            channels
                .iter()
                .find(|(_, permissions)| can_send(*permissions))
                .map(|(id, _)| *id)
        })
}

/// 案内の言語。保存済みのサーバー言語があればそれ、無ければ Discord のサーバー言語
pub fn welcome_locale(saved: Option<&str>, preferred: &str) -> Locale {
    Locale::resolve(saved.unwrap_or(preferred))
}

/// 案内の文面
#[derive(Debug, PartialEq, Eq)]
pub struct WelcomeText {
    pub title: String,
    pub description: String,
    pub calendar_label: &'static str,
    pub docs_label: &'static str,
}

pub fn welcome_text(locale: Locale) -> WelcomeText {
    match locale {
        Locale::Ja => WelcomeText {
            title: "DisCalendar を導入していただきありがとうございます".to_owned(),
            description: [
                "このサーバーの共有カレンダーが使えるようになりました。",
                "",
                "**1. 予定を作る**",
                "下の「カレンダーを開く」から Web で作成できます。Discord では `/create` でも作れます。",
                "",
                "**2. 通知を受け取るチャンネルを決める**",
                "予定の時刻に通知を送ってほしいチャンネルで `/init` を実行してください。",
                "",
                "コマンドの一覧は `/help`、詳しい使い方は「使い方」から確認できます。",
            ]
            .join("\n"),
            calendar_label: "カレンダーを開く",
            docs_label: "使い方",
        },
        Locale::En => WelcomeText {
            title: "Thanks for adding DisCalendar!".to_owned(),
            description: [
                "This server now has a shared calendar.",
                "",
                "**1. Create an event**",
                "Use \"Open calendar\" below to create events on the web, or run `/create` in Discord.",
                "",
                "**2. Choose a channel for reminders**",
                "Run `/init` in the channel where you want event reminders to be posted.",
                "",
                "See `/help` for all commands, or \"Guide\" for detailed instructions.",
            ]
            .join("\n"),
            calendar_label: "Open calendar",
            docs_label: "Guide",
        },
    }
}

fn build_message(
    locale: Locale,
    site_base_url: &str,
    guild_id: GuildId,
) -> serenity::CreateMessage {
    let text = welcome_text(locale);
    let site = site_base_url.trim_end_matches('/');
    let embed = serenity::CreateEmbed::new()
        .title(text.title)
        .description(text.description)
        .colour(0x0000dd);
    let buttons = serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new_link(format!("{site}/dashboard/{guild_id}"))
            .label(text.calendar_label),
        serenity::CreateButton::new_link(format!("{site}/docs/gettingstarted"))
            .label(text.docs_label),
    ]);
    serenity::CreateMessage::new()
        .embed(embed)
        .components(vec![buttons])
}

/// キャッシュから取り出した送り先の候補と Discord のサーバー言語
struct Candidates {
    system_channel: Option<ChannelId>,
    /// テキストチャンネルを並び順に並べ、Bot の権限を添えたもの
    channels: Vec<(ChannelId, Permissions)>,
    preferred_locale: String,
}

/// キャッシュから送り先の候補を取り出す。
/// キャッシュの参照 (内部ロック) は await をまたがないよう、ここで手放す
fn channel_candidates(ctx: &serenity::Context, guild_id: GuildId) -> Option<Candidates> {
    let bot_id = ctx.cache.current_user().id;
    let guild = ctx.cache.guild(guild_id)?;
    // 参加時の GuildCreate には Bot 自身のメンバー情報が含まれる
    let member = guild.members.get(&bot_id)?;
    let mut channels: Vec<_> = guild
        .channels
        .values()
        .filter(|channel| channel.kind == ChannelType::Text)
        .collect();
    channels.sort_by_key(|channel| (channel.position, channel.id));
    let channels = channels
        .into_iter()
        .map(|channel| (channel.id, guild.user_permissions_in(channel, member)))
        .collect();
    Some(Candidates {
        system_channel: guild.system_channel_id,
        channels,
        preferred_locale: guild.preferred_locale.clone(),
    })
}

/// 新しく参加したサーバーに案内を送る。送れなくてもエラーにはせず、ログに残すだけにする
pub async fn send(ctx: &serenity::Context, data: &Data, guild_id: GuildId) {
    let Some(Candidates {
        system_channel,
        channels,
        preferred_locale,
    }) = channel_candidates(ctx, guild_id)
    else {
        tracing::warn!(%guild_id, "guild or bot member not cached, skipping welcome message");
        return;
    };
    let Some(channel_id) = pick_channel(system_channel, &channels) else {
        // 記録はしない (権限を直して追加し直したときに送れるようにする)
        tracing::info!(%guild_id, "no channel to send the welcome message");
        return;
    };

    let guild = guild_id.to_string();
    let now = now_jst();
    match guild_welcome::claim(&data.pool, &guild, now).await {
        Ok(true) => {}
        Ok(false) => {
            tracing::info!(%guild_id, "welcome message was sent recently, skipping");
            return;
        }
        Err(error) => {
            tracing::warn!(%guild_id, %error, "failed to record welcome message");
            return;
        }
    }
    let saved = match guild_welcome::saved_locale(&data.pool, &guild).await {
        Ok(saved) => saved,
        Err(error) => {
            tracing::warn!(%guild_id, %error, "failed to read guild locale for welcome message");
            None
        }
    };
    let locale = welcome_locale(saved.as_deref(), &preferred_locale);
    let message = build_message(locale, &data.site_base_url, guild_id);
    match channel_id.send_message(&ctx.http, message).await {
        Ok(_) => {
            tracing::info!(%guild_id, channel_id = channel_id.get(), locale = locale.as_str(), "sent welcome message");
        }
        Err(error) => {
            tracing::warn!(%guild_id, channel_id = channel_id.get(), %error, "failed to send welcome message");
            if let Err(error) = guild_welcome::release(&data.pool, &guild, now).await {
                tracing::warn!(%guild_id, %error, "failed to release welcome message record");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(id: u64) -> ChannelId {
        ChannelId::new(id)
    }

    fn sendable() -> Permissions {
        required_permissions() | Permissions::READ_MESSAGE_HISTORY
    }

    #[test]
    fn prefers_the_system_channel_when_the_bot_can_post() {
        let channels = [(ch(1), sendable()), (ch(2), sendable())];
        assert_eq!(pick_channel(Some(ch(2)), &channels), Some(ch(2)));
    }

    #[test]
    fn falls_back_to_the_first_sendable_channel() {
        let channels = [
            (ch(1), Permissions::VIEW_CHANNEL),
            // 埋め込みを送れないチャンネルは案内が崩れるので使わない
            (
                ch(2),
                Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
            ),
            (ch(3), sendable()),
            (ch(4), sendable()),
        ];
        // システムチャンネルに投稿できない
        assert_eq!(pick_channel(Some(ch(1)), &channels), Some(ch(3)));
        // システムチャンネルが無い
        assert_eq!(pick_channel(None, &channels), Some(ch(3)));
        // システムチャンネルがテキストチャンネルの一覧に無い (権限が分からない)
        assert_eq!(pick_channel(Some(ch(9)), &channels), Some(ch(3)));
    }

    #[test]
    fn sends_nothing_when_no_channel_is_sendable() {
        let channels = [(ch(1), Permissions::VIEW_CHANNEL)];
        assert_eq!(pick_channel(Some(ch(1)), &channels), None);
        assert_eq!(pick_channel(None, &[]), None);
    }

    #[test]
    fn saved_locale_wins_over_discord_locale() {
        assert_eq!(welcome_locale(Some("ja"), "en-US"), Locale::Ja);
        assert_eq!(welcome_locale(Some("en"), "ja"), Locale::En);
        assert_eq!(welcome_locale(None, "en-US"), Locale::En);
        assert_eq!(welcome_locale(None, "ja"), Locale::Ja);
        // 未対応の言語は日本語
        assert_eq!(welcome_locale(None, "ko"), Locale::Ja);
    }

    #[test]
    fn text_mentions_the_first_steps_in_both_languages() {
        for locale in [Locale::Ja, Locale::En] {
            let text = welcome_text(locale);
            assert!(text.description.contains("`/create`"));
            assert!(text.description.contains("`/init`"));
            assert!(text.description.contains("`/help`"));
            // 埋め込みの説明は 4096 文字まで
            assert!(text.description.chars().count() < 4096);
        }
    }
}
