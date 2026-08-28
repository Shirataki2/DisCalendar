//! 利用者の日次アクティビティ (`user_daily_activity`、#81)。
//!
//! api がセッションを検証したリクエスト (`crate::auth`) を「その利用者がその日 (JST) に使った」として
//! 1 人 1 日 1 行で積み上げる。分析情報 (#79) のアクティブユーザーはセッションの生存期間からの
//! 推定だったが、この記録で正確な DAU / WAU / MAU が出せる (`admin_analytics::measured_active_users`)。
//!
//! - 記録するのは利用者 ID と日付の組だけ。IP アドレス・ユーザーエージェント・操作の内容は持たない
//! - 運営者の閲覧が指標に混ざらないよう、管理コンソール (`/admin/*`) へのリクエストは数えない
//! - 同じ日の 2 回目以降の書き込みは [`crate::state::AppState::activity_days`] のキャッシュでスキップする

use chrono::NaiveDate;
use sqlx::PgExecutor;

/// その利用者がその日に使ったことを記録する (1 人 1 日 1 行。既にあれば何もしない)
pub async fn record<'e>(
    executor: impl PgExecutor<'e>,
    user_id: &str,
    day: NaiveDate,
) -> sqlx::Result<()> {
    sqlx::query!(
        "INSERT INTO user_daily_activity (user_id, day) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        user_id,
        day,
    )
    .execute(executor)
    .await?;
    Ok(())
}
