//! 個人返信は Discord の利用者言語、チャンネル投稿は保存済みサーバー言語。
#[path = "../../shared/i18n.rs"]
mod shared;
use crate::data::Context;
pub use shared::Locale;

pub fn user_locale(ctx: Context<'_>) -> Locale {
    match ctx {
        Context::Application(ctx) => Locale::resolve(&ctx.interaction.locale),
        Context::Prefix(_) => Locale::Ja,
    }
}

pub async fn guild_locale(pool: &sqlx::PgPool, guild: &str) -> sqlx::Result<Locale> {
    Ok(crate::models::guilds::find_by_guild_id(pool, guild)
        .await?
        .map(|guild| Locale::resolve(&guild.locale))
        .unwrap_or_default())
}
