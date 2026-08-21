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

fn is_snowflake(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit())
}

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
}
