//! 登録月ごとの新規ユーザーの定着 (#398)。管理コンソールの分析情報 (`GET /admin/analytics`) で出す。
//!
//! 導入からの定着 (ギルド、[`super::guild_retention`]) の利用者版。web にログインした利用者を
//! 登録日 (`user.createdAt`、JST) の月で分け、月ごとに次を数える。
//!
//! - 登録の翌日から 7 日目までに、また使った (`user_daily_activity` に行がある) 人
//! - 8 日目から 30 日目までに使った人 (最初の 1 週間を過ぎても戻ってきたか)
//! - 直近 [`super::admin_analytics::RECENT_DAYS`] 日に使っている人
//!
//! # 定義
//!
//! - 登録日当日の利用は数えない (登録した時点で必ず 1 回使っているので、定着の判定にならない)
//! - 7 日 / 30 日の割合の分母は、**その日数が今日より前に過ぎ切った人だけ** (`*_eligible`)。
//!   今日はまだ途中なので、`登録日 + N < 今日` の人に限る。登録したばかりの人を分母に入れると、
//!   まだ戻ってくる時間が無いのに「戻らなかった」側に数えてしまう
//! - 利用の記録 (#81) が始まった日 ([`UserRetention::since`]) より前に登録した人は、登録直後の利用が
//!   分からないので数えない。記録開始の月は、その日以降に登録した人だけになる
//!
//! # 精度の限界 (画面にも注記する)
//!
//! - 退会 (利用者情報の削除) した人は `user` から行が消えるので数えられない
//! - 日付は JST の暦日で区切るので、登録が深夜なら数分後の利用でも「翌日」になる

use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;

/// 登録月 1 つぶんの定着状況
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
pub struct UserRetentionCohort {
    /// 登録月 (JST の月初)
    #[schema(example = "2026-10-01")]
    pub month: NaiveDate,
    /// その月に登録した (今も残っている) 利用者の数
    #[schema(example = 120)]
    pub users: i64,
    /// そのうち登録から 7 日が過ぎ切った人の数 (7 日目までの割合の分母)
    #[schema(example = 100)]
    pub week_eligible: i64,
    /// [`UserRetentionCohort::week_eligible`] のうち、登録の翌日〜7 日目に使った人の数
    #[schema(example = 40)]
    pub week_returned: i64,
    /// そのうち登録から 30 日が過ぎ切った人の数 (8〜30 日目の割合の分母)
    #[schema(example = 60)]
    pub month_eligible: i64,
    /// [`UserRetentionCohort::month_eligible`] のうち、登録の 8〜30 日目に使った人の数
    #[schema(example = 20)]
    pub month_returned: i64,
    /// 直近 N 日に使っている人の数 (分母は [`UserRetentionCohort::users`])
    #[schema(example = 30)]
    pub active: i64,
}

/// 登録月ごとの定着状況
#[derive(Debug, Serialize, ToSchema)]
pub struct UserRetention {
    /// 利用の記録が始まった日。これより前に登録した人は数えていない。記録がまだ無ければ null
    #[schema(example = "2026-08-28")]
    pub since: Option<NaiveDate>,
    /// 登録月の新しい順。登録した人がいない月は含まない
    pub cohorts: Vec<UserRetentionCohort>,
}

/// 登録月ごとの定着状況を `now` (JST) 基準で集計する。`now` より後の登録・利用は数えない
pub async fn user_retention(
    conn: &mut PgConnection,
    now: NaiveDateTime,
    recent_days: i64,
) -> sqlx::Result<UserRetention> {
    let today = now.date();
    // 期間が過ぎ切った人: 登録日 + N < 今日
    let week_due = today - Duration::days(7);
    let month_due = today - Duration::days(30);
    let recent_since = today - Duration::days(recent_days);

    let since: Option<NaiveDate> = sqlx::query_scalar!(
        "SELECT min(day) FROM user_daily_activity WHERE day <= $1",
        today
    )
    .fetch_one(&mut *conn)
    .await?;
    let Some(since) = since else {
        return Ok(UserRetention {
            since: None,
            cohorts: Vec::new(),
        });
    };

    // 利用の記録は 1 回だけ走査する: 登録者ごとに、各期間に使ったかを bool_or でまとめる
    let rows = sqlx::query!(
        r#"
        WITH registered AS (
            SELECT id, ("createdAt" AT TIME ZONE 'Asia/Tokyo')::date AS reg
            FROM "user"
            WHERE "createdAt" >= $2::date::timestamp AT TIME ZONE 'Asia/Tokyo'
              AND "createdAt" <= $1::timestamp AT TIME ZONE 'Asia/Tokyo'
        ),
        used AS (
            SELECT r.id,
                   bool_or(a.day > r.reg AND a.day <= r.reg + 7) AS week,
                   bool_or(a.day > r.reg + 7 AND a.day <= r.reg + 30) AS month,
                   bool_or(a.day > $5::date) AS recent
            FROM registered r
            JOIN user_daily_activity a ON a.user_id = r.id AND a.day <= $1::timestamp::date
            GROUP BY r.id
        )
        SELECT
            date_trunc('month', r.reg)::date AS "month!",
            count(*) AS "users!",
            count(*) FILTER (WHERE r.reg < $3::date) AS "week_eligible!",
            count(*) FILTER (WHERE r.reg < $3::date AND u.week) AS "week_returned!",
            count(*) FILTER (WHERE r.reg < $4::date) AS "month_eligible!",
            count(*) FILTER (WHERE r.reg < $4::date AND u.month) AS "month_returned!",
            count(*) FILTER (WHERE u.recent) AS "active!"
        FROM registered r
        LEFT JOIN used u ON u.id = r.id
        GROUP BY 1
        ORDER BY 1 DESC
        "#,
        now,
        since,
        week_due,
        month_due,
        recent_since,
    )
    .fetch_all(&mut *conn)
    .await?;

    Ok(UserRetention {
        since: Some(since),
        cohorts: rows
            .into_iter()
            .map(|row| UserRetentionCohort {
                month: row.month,
                users: row.users,
                week_eligible: row.week_eligible,
                week_returned: row.week_returned,
                month_eligible: row.month_eligible,
                month_returned: row.month_returned,
                active: row.active,
            })
            .collect(),
    })
}
