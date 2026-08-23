use std::collections::HashSet;

use sqlx::PgPool;

use crate::{auth::AuthConfig, discord::DiscordClient};

/// 全ハンドラで共有する状態 (`web::Data<AppState>`)
pub struct AppState {
    pub pool: PgPool,
    /// SQL コンソール (#36) 専用のプール。権限を絞ったロール `discalendar_sql_console` でログインする
    /// (`models::admin_sql`)。接続は遅延なので、ロールが無い環境でも起動はできる
    pub sql_console_pool: PgPool,
    pub discord: DiscordClient,
    pub auth: AuthConfig,
    pub admin: AdminConfig,
}

/// 管理コンソール (`/admin/*`) の設定
#[derive(Debug, Clone, Default)]
pub struct AdminConfig {
    /// 管理者として扱う Discord ユーザー ID (`ADMIN_DISCORD_USER_IDS`)。空なら誰も管理者ではない
    pub discord_user_ids: HashSet<String>,
}

impl AdminConfig {
    pub fn is_admin(&self, discord_user_id: &str) -> bool {
        self.discord_user_ids.contains(discord_user_id)
    }
}
