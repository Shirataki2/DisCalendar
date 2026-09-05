use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

/// OpenAPI ドキュメント。paths / schemas は `utoipa_actix_web` がルート登録時に集める。
/// `/openapi.json` で取得でき、`/docs/` に Swagger UI がある
#[derive(OpenApi)]
#[openapi(
    info(
        title = "DisCalendar API",
        description = "Discord 用共有カレンダー DisCalendar の REST API。\
            認証は web (Better Auth) のセッション cookie。日時はすべてタイムゾーンなしの JST。"
    ),
    tags(
        (name = "meta", description = "バージョン / ヘルスチェック"),
        (name = "guilds", description = "ギルド情報・権限・設定"),
        (name = "events", description = "予定の CRUD"),
        (name = "shares", description = "予定の共有リンク"),
        (name = "feeds", description = "iCal フィード (外部カレンダーからの購読)"),
        (name = "admin", description = "管理コンソール (ADMIN_DISCORD_USER_IDS のユーザーのみ)"),
    ),
    modifiers(&SecurityAddon),
    security(("session" = []))
)]
pub struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                "better-auth.session_token",
                "Better Auth のセッション cookie (本番では __Secure- 付き)。\
                 curl などからは同じ値を `Authorization: Bearer <value>` で渡してもよい",
            ))),
        );
    }
}
