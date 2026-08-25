//! 管理コンソールの分析情報 (`GET /admin/analytics`、#79)。
//!
//! 概要 (`/admin/stats`) とは別のエンドポイントにしている。概要は障害時に最初に開く画面なので、
//! 集計の重いクエリを混ぜて道連れにしないため。読み取りだけなので監査ログには残さない。
//!
//! 指標の定義と精度の限界は [`crate::models::admin_analytics`] のモジュールコメントを参照。

use actix_web::{get, web};
use chrono::{Duration, NaiveDate, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    admin::AdminUser,
    error::{ApiError, ErrorBody},
    models::{
        admin_analytics::{
            self, ActiveUsers, Breakdown, DAILY_DAYS, DailyPoint, EventCreation, GuildActivity,
            MONTHLY_MONTHS, MonthlyPoint, RECENT_DAYS,
        },
        now_jst,
    },
    state::AppState,
};

/// 分析情報 (`GET /admin/analytics`)
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminAnalytics {
    /// 集計の基準にした日 (JST)。推移の右端の日付
    #[schema(example = "2026-08-25")]
    pub today: NaiveDate,
    /// 日別の推移が何日ぶんか
    #[schema(example = 30)]
    pub daily_days: i32,
    /// 月別の推移が何ヶ月ぶんか
    #[schema(example = 12)]
    pub monthly_months: i32,
    /// 「アクティブ」「直近」の基準にした日数
    #[schema(example = 30)]
    pub recent_days: i64,
    pub active_users: ActiveUsers,
    pub event_creation: EventCreation,
    pub breakdown: Breakdown,
    pub guilds: GuildActivity,
    /// 日別の推移 (古い順、[`AdminAnalytics::daily_days`] 件)
    pub daily: Vec<DailyPoint>,
    /// 月別の推移 (古い順、[`AdminAnalytics::monthly_months`] 件)
    pub monthly: Vec<MonthlyPoint>,
}

/// 分析情報。アクティブユーザー・予定の作成数・ギルドの利用状況とその推移をまとめて返す。
///
/// `events` を数回走査するので `/admin/stats` より重い。管理者しか開かないため索引は足していない
#[utoipa::path(
    tag = "admin",
    responses(
        (status = 200, body = AdminAnalytics),
        (status = 401, body = ErrorBody),
        (status = 403, body = ErrorBody),
    )
)]
#[get("/analytics")]
pub async fn analytics(
    _admin: AdminUser,
    state: web::Data<AppState>,
) -> Result<web::Json<AdminAnalytics>, ApiError> {
    let now_utc = Utc::now();
    let now = now_jst();
    let recent_since = now - Duration::days(RECENT_DAYS);

    let (active_users, daily, monthly, events, guilds, settings) = tokio::try_join!(
        admin_analytics::active_users(&state.pool, now_utc),
        admin_analytics::daily(&state.pool, now.date()),
        admin_analytics::monthly(&state.pool, now_utc),
        admin_analytics::events(&state.pool, now),
        admin_analytics::guild_activity(&state.pool, recent_since),
        admin_analytics::guild_settings(&state.pool),
    )?;

    let (event_creation, all_day_events, events_with_notifications, notifications_per_event) =
        events;
    let (guilds_with_channel, restricted_guilds) = settings;

    Ok(web::Json(AdminAnalytics {
        today: now.date(),
        daily_days: DAILY_DAYS,
        monthly_months: MONTHLY_MONTHS,
        recent_days: RECENT_DAYS,
        active_users,
        event_creation,
        breakdown: Breakdown {
            all_day_events,
            events_with_notifications,
            notifications_per_event,
            guilds_with_channel,
            restricted_guilds,
        },
        guilds,
        daily,
        monthly,
    }))
}
