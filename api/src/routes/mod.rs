//! ルーティング。`utoipa_actix_web` 経由で登録することで OpenAPI にも自動で載る。

mod admin;
mod admin_analytics;
mod admin_audit;
mod admin_guilds;
mod admin_ops;
mod admin_sql;
mod admin_status;
mod admin_users;
mod events;
mod feeds;
mod guilds;
mod health;
mod member;
mod shares;

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
                .service(guilds::refresh_my_permissions)
                .service(guilds::get_config)
                .service(guilds::put_config)
                // iCal フィードの発行状況と発行・無効化 (#95)
                .service(feeds::get_feed)
                .service(feeds::issue_feed)
                .service(feeds::revoke_feed),
        )
        // iCal フィードの配信 (#95)。認証なしのデータ取得ルート (トークンが URL に入る)
        .service(feeds::download_feed)
        .service(shares::public_share)
        .service(
            scope("/events")
                // `/@me` (横断カレンダー #98) は `/{guild_id}` より先に登録しないと guild_id として解釈される
                .service(events::list_joined)
                .service(shares::get_share)
                .service(shares::issue_share)
                .service(shares::revoke_share)
                .service(events::list)
                .service(events::create)
                .service(events::update)
                .service(events::delete),
        )
        // 管理コンソール。各ハンドラが AdminUser を要求する (#34)
        .service(
            scope("/admin")
                .service(admin::me)
                .service(admin_status::stats)
                .service(admin_analytics::analytics)
                .service(admin_status::status)
                // `/guilds/sync-check` は `/guilds/{guild_id}` より先に登録する
                // (後だと sync-check が guild_id として解釈される)
                .service(admin_status::sync_check)
                .service(admin_guilds::list_guilds)
                .service(admin_guilds::get_guild)
                .service(admin_guilds::list_events)
                .service(admin_guilds::create_event)
                .service(admin_guilds::update_event)
                .service(admin_guilds::delete_event)
                .service(admin_guilds::put_config)
                .service(admin_sql::run_sql)
                .service(admin_sql::history)
                .service(admin_ops::delete_guild_events)
                .service(admin_ops::purge_expired_sessions)
                .service(admin_users::list_users)
                .service(admin_users::list_sessions)
                .service(admin_users::revoke_sessions)
                .service(admin_audit::list_audit_logs),
        );
}
