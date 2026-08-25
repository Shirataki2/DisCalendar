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
    /// 集計の基準にした日 (JST)。推移の右端の日付で、その日は**まだ途中**
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
    /// 日別の推移 (古い順、[`AdminAnalytics::daily_days`] 件)。末尾は今日 (集計時点まで)
    pub daily: Vec<DailyPoint>,
    /// 月別の推移 (古い順、[`AdminAnalytics::monthly_months`] 件)。末尾は当月 (途中まで)
    pub monthly: Vec<MonthlyPoint>,
}

/// 分析情報。アクティブユーザー・予定の作成数・ギルドの利用状況とその推移をまとめて返す。
///
/// 集計は**読み取り専用の REPEATABLE READ トランザクション 1 本**で行う。
///
/// - 接続を 1 本しか使わないので、並列に投げて既定 5 本のプールを埋め、
///   通常の予定 API を接続待ちにすることがない
/// - すべてのクエリが同じスナップショットを読むので、集計の途中で予定が作成・削除されても
///   数字どうしが食い違わない (総数を分母にした割合が 100% を超える、など)
///
/// 個々のクエリは期間で絞った 1 回の集計に収めてある (`models::admin_analytics` を参照)
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
    // 集計中に予定が作られても数字が食い違わないよう、全部を同じスナップショットで読む。
    // 書き込みはしないので READ ONLY にしておく (SQL コンソールの `BEGIN READ ONLY` と同じ考え方)
    let mut tx = state
        .pool
        .begin_with("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await?;

    let active_users = admin_analytics::active_users(&mut *tx, now_utc).await?;
    let daily = admin_analytics::daily(&mut *tx, now.date()).await?;
    let monthly = admin_analytics::monthly(&mut *tx, now_utc).await?;
    let (event_creation, all_day_events) = admin_analytics::event_creation(&mut *tx, now).await?;
    let (events_with_notifications, notification_total) =
        admin_analytics::notification_stats(&mut *tx).await?;
    let (active_guilds, active_left_guilds, joined_guilds) =
        admin_analytics::guild_counts(&mut *tx, recent_since, now).await?;
    let top_guilds = admin_analytics::top_guilds(&mut *tx, recent_since, now).await?;
    let (guilds_with_channel, restricted_guilds) =
        admin_analytics::guild_settings(&mut *tx).await?;
    // 読むだけなので commit / rollback のどちらでもよいが、スナップショットは早く手放す
    tx.rollback().await?;

    let notifications_per_event = if event_creation.total == 0 {
        0.0
    } else {
        notification_total as f64 / event_creation.total as f64
    };

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
        guilds: GuildActivity {
            active_guilds,
            active_left_guilds,
            joined_guilds,
            top_guilds,
        },
        daily,
        monthly,
    }))
}
