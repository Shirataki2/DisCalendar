//! 管理コンソール (`/admin/*`) の認可。
//!
//! 認証 (Better Auth のセッション) は通常の API と同じ [`AuthUser`] で行い、その上で
//! 連携済み Discord アカウントのユーザー ID が `ADMIN_DISCORD_USER_IDS` に含まれるかで判定する。
//! ギルドのメンバーシップ ([`crate::routes::GuildMember`]) は見ないので、管理者は所属していない
//! ギルドのデータも扱える。web 側の表示制御とは独立に、必ずこの extractor で拒否する。

use std::{future::Future, pin::Pin};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web};
use anyhow::anyhow;

use crate::{auth::AuthUser, error::ApiError, state::AppState};

/// ホワイトリストに含まれる認証済みユーザー。ハンドラの引数に書くだけで、
/// 未認証なら 401、管理者でなければ 403 が返る
#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthUser);

impl std::ops::Deref for AdminUser {
    type Target = AuthUser;

    fn deref(&self) -> &AuthUser {
        &self.0
    }
}

impl FromRequest for AdminUser {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, ApiError>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let user_fut = AuthUser::from_request(req, payload);
        let req = req.clone();
        Box::pin(async move {
            let user = user_fut.await?;
            let state = req
                .app_data::<web::Data<AppState>>()
                .ok_or_else(|| anyhow!("AppState is not registered"))?;
            if !state.admin.is_admin(&user.discord_user_id) {
                // 誰が拒否されたかは運用上知りたいのでログには残す (レスポンスには出さない)
                tracing::warn!(
                    user_id = %user.id,
                    discord_user_id = %user.discord_user_id,
                    path = %req.path(),
                    "non-admin user tried to access the admin API"
                );
                return Err(ApiError::Forbidden("administrator only".into()));
            }
            Ok(AdminUser(user))
        })
    }
}
