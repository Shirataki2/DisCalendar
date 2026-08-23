use anyhow::{Context as _, Result};

/// 環境変数から読む設定。開発時は `api/.env` (dotenvy) から読み込まれる
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub max_db_connections: u32,
    /// Discord Bot トークン。メンバー確認・権限計算に使う
    pub discord_bot_token: String,
    /// web 側と同じ値。Better Auth のセッション cookie の署名検証に使う
    pub better_auth_secret: String,
    /// Better Auth の cookie プレフィックス (既定 `better-auth`)
    pub auth_cookie_prefix: String,
    /// 管理コンソール (`/admin/*`) を使える Discord ユーザー ID (`ADMIN_DISCORD_USER_IDS`、カンマ区切り)。
    /// 空なら管理者はいない (全員 403)
    pub admin_discord_user_ids: Vec<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: env_or("HOST", "0.0.0.0"),
            port: env_or("PORT", "8080")
                .parse()
                .context("PORT must be a number")?,
            database_url: required("DATABASE_URL")?,
            max_db_connections: env_or("DATABASE_MAX_CONNECTIONS", "5")
                .parse()
                .context("DATABASE_MAX_CONNECTIONS must be a number")?,
            discord_bot_token: required("DISCORD_BOT_TOKEN")?,
            better_auth_secret: required("BETTER_AUTH_SECRET")?,
            auth_cookie_prefix: env_or("AUTH_COOKIE_PREFIX", "better-auth"),
            admin_discord_user_ids: parse_admin_discord_user_ids(&env_or(
                "ADMIN_DISCORD_USER_IDS",
                "",
            ))?,
        })
    }

    /// Better Auth が使うセッション cookie 名。
    /// baseURL が https のとき (本番) は `__Secure-` プレフィックスが付くので両方受け付ける
    pub fn session_cookie_names(&self) -> [String; 2] {
        [
            format!("__Secure-{}.session_token", self.auth_cookie_prefix),
            format!("{}.session_token", self.auth_cookie_prefix),
        ]
    }
}

/// `ADMIN_DISCORD_USER_IDS` (カンマ区切り) を ID の一覧にする。空要素は無視し、
/// Snowflake でない値が混ざっていたら設定ミスとして起動時に失敗させる
/// (typo で誰も管理者になれない / 意図しない値が通るのを防ぐ)
fn parse_admin_discord_user_ids(raw: &str) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for id in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        anyhow::ensure!(
            // routes::member::is_snowflake と同じ基準
            id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit()),
            "ADMIN_DISCORD_USER_IDS must be comma-separated Discord user IDs (snowflakes), got {id:?}"
        );
        if !ids.iter().any(|existing| existing == id) {
            ids.push(id.to_owned());
        }
    }
    Ok(ids)
}

fn required(key: &str) -> Result<String> {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .with_context(|| format!("{key} must be set"))
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comma_separated_ids_ignoring_blanks_and_duplicates() {
        let ids = parse_admin_discord_user_ids(
            " 123456789012345678, ,987654321098765432,123456789012345678 ",
        )
        .unwrap();
        assert_eq!(ids, vec!["123456789012345678", "987654321098765432"]);
    }

    #[test]
    fn empty_means_no_admins() {
        assert!(parse_admin_discord_user_ids("").unwrap().is_empty());
        assert!(parse_admin_discord_user_ids(" , ").unwrap().is_empty());
    }

    #[test]
    fn rejects_non_snowflake_values() {
        assert!(parse_admin_discord_user_ids("tomoya@example.com").is_err());
        assert!(parse_admin_discord_user_ids("123456789012345678,abc").is_err());
        assert!(parse_admin_discord_user_ids("123456789012345678901").is_err());
    }
}
