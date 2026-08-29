use std::{future::Future, pin::Pin};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web};
use anyhow::anyhow;

use crate::{
    auth::AuthUser,
    discord::{MemberAccess, Permissions},
    error::ApiError,
    state::AppState,
};

/// パスの `{guild_id}` のギルドに認証済みユーザーが所属していることを保証する extractor。
///
/// - 未認証 → 401
/// - guild_id が Snowflake でない → 400
/// - Bot が未参加 / ユーザーが非メンバー → 403
#[derive(Debug, Clone)]
pub struct GuildMember {
    pub user: AuthUser,
    pub access: MemberAccess,
}

impl GuildMember {
    pub fn guild_id(&self) -> &str {
        &self.access.guild.id
    }

    pub fn permissions(&self) -> Permissions {
        self.access.permissions
    }
}

impl FromRequest for GuildMember {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, ApiError>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let user_fut = AuthUser::from_request(req, payload);
        let req = req.clone();
        Box::pin(async move {
            let user = user_fut.await?;
            let guild_id = req
                .match_info()
                .get("guild_id")
                .ok_or_else(|| anyhow!("route has no {{guild_id}} segment"))?
                .to_owned();
            if !is_snowflake(&guild_id) {
                return Err(ApiError::BadRequest("guild_id must be a snowflake".into()));
            }
            let state = req
                .app_data::<web::Data<AppState>>()
                .ok_or_else(|| anyhow!("AppState is not registered"))?;
            let access = state
                .discord
                .member_access(&guild_id, &user.discord_user_id)
                .await?
                .ok_or_else(|| {
                    ApiError::Forbidden(
                        "you are not a member of this guild, or the bot has not joined it".into(),
                    )
                })?;
            Ok(GuildMember { user, access })
        })
    }
}

/// Discord の Snowflake ID (数字のみ、20 桁以下) か。
/// 実体は `discord` 側 (URL の組み立てでも同じ基準で確認する) にある
pub(crate) use crate::discord::is_snowflake;

#[cfg(test)]
mod tests {
    use super::is_snowflake;

    #[test]
    fn snowflake_validation() {
        assert!(is_snowflake("782502586817314816"));
        assert!(!is_snowflake(""));
        assert!(!is_snowflake("abc"));
        assert!(!is_snowflake("123456789012345678901"));
        assert!(!is_snowflake("-1"));
    }

    #[test]
    fn snowflake_validation_rejects_url_control_characters() {
        // Discord API の URL に埋め込む値なので、パスの意味を変えうる文字は通さない
        for id in [
            "1/2",
            "..",
            "../../users/@me",
            "1?query=x",
            "1#frag",
            "1%2F2",
            "1 2",
        ] {
            assert!(!is_snowflake(id), "{id}");
        }
    }
}
