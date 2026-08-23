//! 管理コンソールのユーザー・セッション (`/admin/users*`、#37)。
//!
//! Better Auth の `user` / `session` を読み、必要なら強制ログアウト (セッションの全削除) をする。
//! **セッショントークンや Discord のアクセストークンはレスポンスに含めない** (AGENTS.md の P0)。
//! 削除は操作と同じトランザクションで `admin_audit_logs` に残す。

use actix_web::{delete, get, web};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::admin_ops::OpsResult;
use crate::{
    admin::AdminUser,
    error::{ApiError, ErrorBody},
    models::{
        admin_audit::{self, AuditEntry},
        admin_users::{self, MAX_PAGE, PAGE_SIZE, SessionSummary, UserSummary},
    },
    state::AppState,
};

#[derive(Deserialize, IntoParams)]
pub struct UserListQuery {
    /// user.id / Discord ユーザー ID の完全一致か、名前・メールアドレスの部分一致。空なら全件
    #[serde(default)]
    pub q: String,
    /// 1 始まりのページ番号 (上限 1,000,000)
    #[serde(default = "default_page")]
    #[param(example = 1, minimum = 1, maximum = 1_000_000)]
    pub page: i64,
}

fn default_page() -> i64 {
    1
}

/// ユーザー一覧のレスポンス
#[derive(Serialize, ToSchema)]
pub struct AdminUserPage {
    pub items: Vec<UserSummary>,
    /// 検索条件に一致する総件数
    #[schema(example = 123)]
    pub total: i64,
    #[schema(example = 1)]
    pub page: i64,
    #[schema(example = 50)]
    pub page_size: i64,
}

/// ユーザーの一覧・検索
#[utoipa::path(
    tag = "admin",
    params(UserListQuery),
    responses(
        (status = 200, body = AdminUserPage),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/users")]
pub async fn list_users(
    _admin: AdminUser,
    query: web::Query<UserListQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<AdminUserPage>, ApiError> {
    if !(1..=MAX_PAGE).contains(&query.page) {
        return Err(ApiError::BadRequest(format!(
            "page must be between 1 and {MAX_PAGE}"
        )));
    }
    let q = query.q.trim();
    let (items, total) = tokio::try_join!(
        admin_users::list(&state.pool, q, query.page),
        admin_users::count(&state.pool, q),
    )?;
    Ok(web::Json(AdminUserPage {
        items,
        total,
        page: query.page,
        page_size: PAGE_SIZE,
    }))
}

#[derive(Deserialize, IntoParams)]
pub struct UserPath {
    /// Better Auth の `user.id`
    pub user_id: String,
}

/// あるユーザーのセッション一覧 (新しい順)。トークンは含まない
#[utoipa::path(
    tag = "admin",
    params(UserPath),
    responses(
        (status = 200, body = Vec<SessionSummary>),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, description = "存在しないユーザー", body = ErrorBody),
    )
)]
#[get("/users/{user_id}/sessions")]
pub async fn list_sessions(
    _admin: AdminUser,
    path: web::Path<UserPath>,
    state: web::Data<AppState>,
) -> Result<web::Json<Vec<SessionSummary>>, ApiError> {
    ensure_user_exists(&state.pool, &path.user_id).await?;
    let sessions = admin_users::sessions(&state.pool, &path.user_id).await?;
    Ok(web::Json(sessions))
}

/// あるユーザーのセッションをすべて削除する (強制ログアウト)。監査ログは `user.revoke_sessions`。
/// 自分自身に対して実行すると自分もログアウトされる
#[utoipa::path(
    tag = "admin",
    params(UserPath),
    responses(
        (status = 200, body = OpsResult),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
        (status = 404, description = "存在しないユーザー", body = ErrorBody),
    )
)]
#[delete("/users/{user_id}/sessions")]
pub async fn revoke_sessions(
    admin: AdminUser,
    path: web::Path<UserPath>,
    state: web::Data<AppState>,
) -> Result<web::Json<OpsResult>, ApiError> {
    let mut tx = state.pool.begin().await?;
    ensure_user_exists(&mut *tx, &path.user_id).await?;
    let deleted = admin_users::delete_sessions(&mut *tx, &path.user_id).await?;
    admin_audit::record(
        &mut *tx,
        &admin,
        AuditEntry {
            action: "user.revoke_sessions",
            target_type: Some("user"),
            target_id: Some(&path.user_id),
            detail: Some(serde_json::json!({ "deleted": deleted })),
            ..Default::default()
        },
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        user_id = %path.user_id,
        deleted,
        admin = %admin.discord_user_id,
        "sessions revoked by admin"
    );
    Ok(web::Json(OpsResult { deleted }))
}

/// 存在しないユーザー ID に空の結果や 0 件削除を返さず 404 にする
async fn ensure_user_exists<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    user_id: &str,
) -> Result<(), ApiError> {
    if !admin_users::exists(executor, user_id).await? {
        return Err(ApiError::NotFound("user not found".into()));
    }
    Ok(())
}
