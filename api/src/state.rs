use sqlx::PgPool;

use crate::{auth::AuthConfig, discord::DiscordClient};

/// 全ハンドラで共有する状態 (`web::Data<AppState>`)
pub struct AppState {
    pub pool: PgPool,
    pub discord: DiscordClient,
    pub auth: AuthConfig,
}
