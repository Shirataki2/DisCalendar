//! ルーティング。`utoipa_actix_web` 経由で登録することで OpenAPI にも自動で載る。

mod admin;
mod admin_guilds;
mod events;
mod guilds;
mod health;
mod member;

pub use member::GuildMember;
use utoipa_actix_web::{scope, service_config::ServiceConfig};

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(health::index)
        .service(health::healthz)
        .service(
            scope("/guilds")
                // `/joined` は `/{guild_id}` より先に登録しないと guild_id として解釈される
                .service(guilds::joined)
                .service(guilds::get_guild)
                .service(guilds::my_permissions)
                .service(guilds::get_config)
                .service(guilds::put_config),
        )
        .service(
            scope("/events")
                .service(events::list)
                .service(events::create)
                .service(events::update)
                .service(events::delete),
        )
        // 管理コンソール。各ハンドラが AdminUser を要求する (#34)
        .service(
            scope("/admin")
                .service(admin::me)
                .service(admin_guilds::list_guilds)
                .service(admin_guilds::get_guild)
                .service(admin_guilds::list_events)
                .service(admin_guilds::create_event)
                .service(admin_guilds::update_event)
                .service(admin_guilds::delete_event)
                .service(admin_guilds::put_config),
        );
}
