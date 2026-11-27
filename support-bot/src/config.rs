use std::{collections::HashSet, num::NonZeroU64};

use anyhow::{Context as _, Result};
use poise::serenity_prelude::{ChannelId, GuildId, RoleId};

/// 環境変数から読む設定。開発時は `support-bot/.env` (dotenvy) から読み込まれる
#[derive(Debug, Clone)]
pub struct Config {
    /// サポート用 Bot のトークン (本体の Bot とは別のアプリ)。`SUPPORT_BOT_DISCORD_TOKEN`
    /// (compose のルートの .env と同じ名前)、無ければ `DISCORD_BOT_TOKEN` から読む
    pub discord_bot_token: String,
    /// サポート用のデータベース (本体の DB とは別。無ければ起動時に作る)
    pub database_url: String,
    pub max_db_connections: u32,
    pub settings: Settings,
}

/// 起動後も参照する設定 (`Data` に持たせる)
#[derive(Debug, Clone)]
pub struct Settings {
    /// サポートサーバー。これ以外のギルドに入れられたら即退出する
    pub guild_id: GuildId,
    /// 記録するチャンネル (フォーラムとテキストチャンネル)。フォーラムは配下の投稿 (スレッド) を記録する
    pub record_channel_ids: HashSet<ChannelId>,
    /// スタッフとして扱うロール。このどれかを持つ人の発言を「スタッフの返信」とみなす
    pub staff_role_ids: HashSet<RoleId>,
    /// 未返信の通知・リマインド・日次まとめの投稿先
    pub staff_channel_id: ChannelId,
    /// 本番に出た版の更新内容を告知するチャンネル (#430)。アナウンスチャンネルなら公開 (crosspost) もする。
    /// 未設定なら告知しない (`SUPPORT_RELEASE_CHANNEL_ID`)
    pub release_channel_id: Option<ChannelId>,
    /// Claude API による要約・分類・返信案 (#426)。API キーが未設定なら None で、ボタンを無効にする
    pub claude: Option<ClaudeSettings>,
    /// チケットからの GitHub Issue の起票 (#426)。トークンが未設定なら None で、ボタンを無効にする
    pub github: Option<GithubSettings>,
}

/// 既定のモデル。`SUPPORT_CLAUDE_MODEL` で変えられる
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-opus-5-5";

#[derive(Debug, Clone)]
pub struct ClaudeSettings {
    pub api_key: Secret,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct GithubSettings {
    /// issues:write だけの fine-grained トークン
    pub token: Secret,
    /// 起票先 (`owner/name`)
    pub repo: String,
}

/// API キー・トークン。`Debug` (ログやパニックのメッセージ) に値が出ないよう包む
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            // 本体の Bot のトークン (DISCORD_BOT_TOKEN) と取り違えないよう、専用の名前を先に見る
            discord_bot_token: optional("SUPPORT_BOT_DISCORD_TOKEN")
                .or_else(|| optional("DISCORD_BOT_TOKEN"))
                .context("SUPPORT_BOT_DISCORD_TOKEN (or DISCORD_BOT_TOKEN) must be set")?,
            database_url: required("DATABASE_URL")?,
            max_db_connections: env_or("DATABASE_MAX_CONNECTIONS", "5")
                .parse()
                .context("DATABASE_MAX_CONNECTIONS must be a number")?,
            settings: Settings {
                guild_id: GuildId::from(snowflake(
                    "SUPPORT_GUILD_ID",
                    &required("SUPPORT_GUILD_ID")?,
                )?),
                record_channel_ids: snowflake_list("SUPPORT_RECORD_CHANNEL_IDS")?
                    .into_iter()
                    .map(ChannelId::from)
                    .collect(),
                staff_role_ids: snowflake_list("SUPPORT_STAFF_ROLE_IDS")?
                    .into_iter()
                    .map(RoleId::from)
                    .collect(),
                staff_channel_id: ChannelId::from(snowflake(
                    "SUPPORT_STAFF_CHANNEL_ID",
                    &required("SUPPORT_STAFF_CHANNEL_ID")?,
                )?),
                release_channel_id: optional("SUPPORT_RELEASE_CHANNEL_ID")
                    .map(|v| snowflake("SUPPORT_RELEASE_CHANNEL_ID", &v))
                    .transpose()?
                    .map(ChannelId::from),
                claude: optional("SUPPORT_ANTHROPIC_API_KEY")
                    .or_else(|| optional("ANTHROPIC_API_KEY"))
                    .map(|api_key| ClaudeSettings {
                        api_key: Secret::new(api_key),
                        model: env_or("SUPPORT_CLAUDE_MODEL", DEFAULT_CLAUDE_MODEL),
                    }),
                github: optional("SUPPORT_GITHUB_TOKEN")
                    .map(|token| -> Result<GithubSettings> {
                        let repo = required("SUPPORT_GITHUB_REPO").context(
                            "SUPPORT_GITHUB_REPO is required when SUPPORT_GITHUB_TOKEN is set",
                        )?;
                        Ok(GithubSettings {
                            token: Secret::new(token),
                            repo: parse_repo(&repo)?,
                        })
                    })
                    .transpose()?,
            },
        })
    }
}

/// `owner/name` の形か確かめる (URL に埋め込むので、英数字と `-` `_` `.` だけを許す)
fn parse_repo(value: &str) -> Result<String> {
    let value = value.trim();
    let valid_part = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    match value.split_once('/') {
        Some((owner, name)) if valid_part(owner) && valid_part(name) => Ok(value.to_owned()),
        _ => anyhow::bail!("SUPPORT_GITHUB_REPO must be in the form owner/name"),
    }
}

fn snowflake(key: &str, value: &str) -> Result<NonZeroU64> {
    value
        .trim()
        .parse::<NonZeroU64>()
        .with_context(|| format!("{key} must be a Discord ID (snowflake)"))
}

/// カンマ区切りの ID 一覧。空は許さない (記録先やスタッフが無いと何もできないので設定漏れとして止める)
fn snowflake_list(key: &str) -> Result<Vec<NonZeroU64>> {
    let value = required(key)?;
    parse_snowflake_list(key, &value)
}

fn parse_snowflake_list(key: &str, value: &str) -> Result<Vec<NonZeroU64>> {
    let ids = value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| snowflake(key, v))
        .collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(!ids.is_empty(), "{key} must contain at least one ID");
    Ok(ids)
}

fn required(key: &str) -> Result<String> {
    optional(key).with_context(|| format!("{key} must be set"))
}

fn optional(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}

fn env_or(key: &str, default: &str) -> String {
    optional(key).unwrap_or_else(|| default.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comma_separated_ids() {
        let ids = parse_snowflake_list("K", " 1, 2 ,,3 ").unwrap();
        assert_eq!(ids.iter().map(|v| v.get()).collect::<Vec<_>>(), [1, 2, 3]);
    }

    #[test]
    fn validates_github_repo() {
        assert_eq!(
            parse_repo(" Shirataki2/DisCalendar ").unwrap(),
            "Shirataki2/DisCalendar"
        );
        assert_eq!(parse_repo("a-b/c_d.e").unwrap(), "a-b/c_d.e");
        for invalid in [
            "", "owner", "owner/", "/name", "a/b/c", "a/..", "a/b?x=1", "a b/c",
        ] {
            assert!(parse_repo(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn secrets_are_hidden_from_debug() {
        let secret = Secret::new("sk-ant-should-not-leak");
        assert_eq!(format!("{secret:?}"), "Secret(***)");
        assert_eq!(secret.expose(), "sk-ant-should-not-leak");
    }

    #[test]
    fn rejects_empty_and_invalid_lists() {
        assert!(parse_snowflake_list("K", " , ").is_err());
        assert!(parse_snowflake_list("K", "1,abc").is_err());
        assert!(parse_snowflake_list("K", "0").is_err());
    }
}
