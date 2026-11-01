//! 招待リンクのクリックの記録 (`POST /invite-clicks`、#374)。
//!
//! web の `/invite` (Route Handler) がリダイレクトの後に呼ぶ。認証なしのルートで、
//! ブラウザからも `/local/api/invite-clicks` で届くが、`/invite` を開けば同じだけ数が増えるので
//! これを塞いでも水増しは防げない。記録する値は導入元と日付だけで、知らない導入元は `other` にまとめる
//! ([`crate::models::invite_clicks`])。

use actix_web::{HttpResponse, post, web};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::{
    error::{ApiError, ErrorBody},
    models::{invite_clicks, now_jst},
    state::AppState,
};

#[derive(Deserialize, ToSchema)]
pub struct InviteClickInput {
    /// 導入元 (`/invite?from=` の値)。無ければ `direct`、知らない値は `other` として数える
    #[schema(example = "lp-hero")]
    pub source: Option<String>,
}

/// 招待リンクのクリックを導入元ごとに 1 件数える
#[utoipa::path(
    tag = "meta",
    request_body = InviteClickInput,
    responses((status = 204), (status = 500, body = ErrorBody)),
    security(())
)]
#[post("/invite-clicks")]
pub async fn record(
    state: web::Data<AppState>,
    body: web::Json<InviteClickInput>,
) -> Result<HttpResponse, ApiError> {
    let source = invite_clicks::normalize(body.source.as_deref());
    invite_clicks::record(&state.pool, source, now_jst().date()).await?;
    Ok(HttpResponse::NoContent().finish())
}
