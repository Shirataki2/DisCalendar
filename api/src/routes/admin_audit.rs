//! 管理コンソールの監査ログ閲覧 (`GET /admin/audit-logs`、#37)。
//!
//! `/admin/*` の書き込み操作と SQL 実行が残す `admin_audit_logs` (models/admin_audit.rs) を
//! 新しい順に読む。読み取りだけなので、この API 自体は監査ログに残さない。

use actix_web::{get, web};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    admin::AdminUser,
    error::{ApiError, ErrorBody},
    models::admin_audit::{self, AuditLog, MAX_PAGE, PAGE_SIZE},
    state::AppState,
};

#[derive(Deserialize, IntoParams)]
pub struct AuditLogQuery {
    /// 操作の種類での絞り込み (`event.update` など)。空なら絞り込まない
    #[serde(default)]
    pub action: String,
    /// 操作した管理者の Discord ユーザー ID での絞り込み。空なら絞り込まない
    #[serde(default)]
    pub actor: String,
    /// 1 始まりのページ番号 (上限 1,000,000)
    #[serde(default = "default_page")]
    #[param(example = 1, minimum = 1, maximum = 1_000_000)]
    pub page: i64,
}

fn default_page() -> i64 {
    1
}

/// 監査ログ一覧のレスポンス
#[derive(Serialize, ToSchema)]
pub struct AuditLogPage {
    pub items: Vec<AuditLog>,
    /// 絞り込み条件に一致する総件数
    #[schema(example = 123)]
    pub total: i64,
    #[schema(example = 1)]
    pub page: i64,
    #[schema(example = 50)]
    pub page_size: i64,
    /// 記録されている操作の種類 (絞り込みの選択肢)
    pub actions: Vec<String>,
}

/// 監査ログの閲覧 (新しい順)
#[utoipa::path(
    tag = "admin",
    params(AuditLogQuery),
    responses(
        (status = 200, body = AuditLogPage),
        (status = 400, body = ErrorBody),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/audit-logs")]
pub async fn list_audit_logs(
    _admin: AdminUser,
    query: web::Query<AuditLogQuery>,
    state: web::Data<AppState>,
) -> Result<web::Json<AuditLogPage>, ApiError> {
    if !(1..=MAX_PAGE).contains(&query.page) {
        return Err(ApiError::BadRequest(format!(
            "page must be between 1 and {MAX_PAGE}"
        )));
    }
    let action = query.action.trim();
    let actor = query.actor.trim();
    let (items, total, actions) = tokio::try_join!(
        admin_audit::list(&state.pool, action, actor, query.page),
        admin_audit::count(&state.pool, action, actor),
        admin_audit::actions(&state.pool),
    )?;
    Ok(web::Json(AuditLogPage {
        items,
        total,
        page: query.page,
        page_size: PAGE_SIZE,
        actions,
    }))
}
