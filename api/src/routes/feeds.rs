//! iCal フィード (#95)。
//!
//! - `/guilds/{guild_id}/feed`: フィードの発行状況の取得 (メンバーなら誰でも) と、発行・再発行・無効化
//!   (サーバー管理権限が必要)
//! - `/feeds/{token}.ics`: 認証なしの配信。トークンを知っていることが唯一の条件で、restricted モードにも
//!   関わらず全予定を含む (restricted は編集の制限で、閲覧はメンバー全員ができる。URL は管理権限を持つ
//!   メンバーが発行し、共有した相手が読む前提)

use actix_web::{HttpResponse, delete, get, http::header, post, web};
use chrono::Duration;

use super::GuildMember;
use crate::{
    error::{ApiError, ErrorBody},
    ical,
    models::{
        events,
        feed_tokens::{self, FeedToken},
        now_jst,
    },
    state::AppState,
};

/// フィードに含める予定の下限: 今からこの日数より前に終わった予定は含めない
/// (未来の予定はすべて含める。行数がギルドの 1 年分 + 未来に収まる)
pub const FEED_LOOKBACK_DAYS: i64 = 365;

/// 配信の Cache-Control。URL 自体が秘密なので共有キャッシュ (CDN) には乗せず、
/// 同じクライアントの連打だけを短時間吸収する。購読クライアントの取得間隔はこれとは別に各社が決める
const CACHE_CONTROL: &str = "private, max-age=300";

/// 発行済みのフィード。未発行なら `null`
#[utoipa::path(
    tag = "feeds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 200, description = "発行済みならそのトークン、未発行なら null", body = Option<FeedToken>),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / Bot 未参加", body = ErrorBody),
    )
)]
#[get("/{guild_id}/feed")]
pub async fn get_feed(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<Option<FeedToken>>, ApiError> {
    Ok(web::Json(
        feed_tokens::get(&state.pool, member.guild_id()).await?,
    ))
}

/// フィードの発行・再発行。サーバー管理権限 (`can_manage_server`) が必要。
/// 発行済みなら新しいトークンに置き換える (古い URL はその時点で使えなくなる)
#[utoipa::path(
    tag = "feeds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 200, body = FeedToken),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / 管理権限なし", body = ErrorBody),
    )
)]
#[post("/{guild_id}/feed")]
pub async fn issue_feed(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<web::Json<FeedToken>, ApiError> {
    require_manage(&member)?;
    let token = feed_tokens::generate_token();
    Ok(web::Json(
        feed_tokens::upsert(
            &state.pool,
            member.guild_id(),
            &token,
            &member.user.discord_user_id,
            now_jst(),
        )
        .await?,
    ))
}

/// フィードの無効化。サーバー管理権限が必要。未発行なら 404
#[utoipa::path(
    tag = "feeds",
    params(("guild_id" = String, Path, description = "ギルド ID")),
    responses(
        (status = 204),
        (status = 401, body = ErrorBody),
        (status = 403, description = "非メンバー / 管理権限なし", body = ErrorBody),
        (status = 404, description = "未発行", body = ErrorBody),
    )
)]
#[delete("/{guild_id}/feed")]
pub async fn revoke_feed(
    member: GuildMember,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    require_manage(&member)?;
    if !feed_tokens::delete(&state.pool, member.guild_id()).await? {
        return Err(ApiError::NotFound("feed is not issued".into()));
    }
    Ok(HttpResponse::NoContent().finish())
}

fn require_manage(member: &GuildMember) -> Result<(), ApiError> {
    if member.permissions().can_manage_server() {
        Ok(())
    } else {
        Err(ApiError::Forbidden(
            "manage permission is required to manage the calendar feed".into(),
        ))
    }
}

/// iCalendar 形式の配信。**認証なし** (トークンを知っていることが条件)。
/// 形の合わないトークン・未発行・Bot が退出済みのギルドはいずれも 404 で、存在の有無は区別しない
#[utoipa::path(
    tag = "feeds",
    params(("token" = String, Path, description = "発行されたトークン (64 文字の 16 進)")),
    responses(
        (status = 200, description = "iCalendar (text/calendar)", content_type = "text/calendar", body = String),
        (status = 404, body = ErrorBody),
    ),
    security(())
)]
#[get("/feeds/{token}.ics")]
pub async fn download_feed(
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let token = path.into_inner();
    let not_found = || ApiError::NotFound("feed not found".into());
    if !feed_tokens::is_token(&token) {
        return Err(not_found());
    }
    let guild = feed_tokens::find_guild_by_token(&state.pool, &token)
        .await?
        .ok_or_else(not_found)?;
    let since = now_jst() - Duration::days(FEED_LOOKBACK_DAYS);
    let rows = events::list_for_feed(&state.pool, &guild.guild_id, since).await?;
    let body = ical::render_feed(&guild, &rows, &state.site_base_url, chrono::Utc::now());
    Ok(HttpResponse::Ok()
        .content_type("text/calendar; charset=utf-8")
        .insert_header((header::CACHE_CONTROL, CACHE_CONTROL))
        .body(body))
}
