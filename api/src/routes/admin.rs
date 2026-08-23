//! 管理コンソール (`/admin/*`)。すべてのハンドラは [`AdminUser`] を要求する。
//! ギルド・予定 (#35)、SQL コンソール (#36)、稼働状況 (#37) はここに足していく。

use actix_web::{get, web};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{admin::AdminUser, error::ErrorBody};

/// 管理者として認識された自分自身の情報
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminMe {
    /// Better Auth の `user.id`
    pub user_id: String,
    pub name: String,
    /// 連携済み Discord アカウントのユーザー ID (Snowflake、文字列)
    #[schema(example = "123456789012345678")]
    pub discord_user_id: String,
}

/// 自分が管理者であることの確認。web の `/admin` はこれで表示可否を決める
/// (非管理者は 403 なので web 側は 404 扱いにする)
#[utoipa::path(
    tag = "admin",
    responses(
        (status = 200, body = AdminMe),
        (status = 401, body = ErrorBody),
        (status = 403, description = "ADMIN_DISCORD_USER_IDS に含まれていない", body = ErrorBody),
    )
)]
#[get("/me")]
pub async fn me(admin: AdminUser) -> web::Json<AdminMe> {
    web::Json(AdminMe {
        user_id: admin.id.clone(),
        name: admin.name.clone(),
        discord_user_id: admin.discord_user_id.clone(),
    })
}
