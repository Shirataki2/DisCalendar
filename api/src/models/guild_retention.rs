//! 導入月ごとの定着状況 (#375)。管理コンソールの分析情報 (`GET /admin/analytics`) で出す。
//!
//! 導入 (Bot の参加) から予定の作成までのどこで離れているかを見るため、参加中のギルドを
//! `guilds.joined_at` の月 (JST) で分け、月ごとに次を数える。
//!
//! - 導入から 7 日 / 30 日以内に予定が作られたギルド
//! - 直近 [`super::admin_analytics::RECENT_DAYS`] 日にも予定が作られている (使い続けている) ギルド
//!
//! # 定義
//!
//! - **予定の作成**は分析情報の他の指標と同じく `event_creations` (単発の予定と、繰り返しの初期シリーズ。
//!   分割先・補充した回は除く) の `created_at`
//! - 「導入から N 日以内」は `joined_at <= created_at <= joined_at + N 日`。**`joined_at` より前の予定は数えない**。
//!   退出直後に再参加したギルドは `joined_at` が再参加の日時に更新される (bot の `upsert_guild`) ので、
//!   前回の参加中に作った予定で「導入してすぐ使った」ことにしないため
//! - 7 日 / 30 日の割合の分母は、**導入から N 日が経ったギルドだけ** (`*_eligible`)。
//!   導入したばかりのギルドを分母に入れると、まだ予定を作る時間が無いのに「作らなかった」側に数えてしまい、
//!   直近の月ほど低く出る
//!
//! # 精度の限界 (画面にも注記する)
//!
//! - 退出したギルドは `guilds` から行が消えるので、導入数は「今も参加しているギルド」になる。
//!   すぐ離れたギルドほど数えられないので、割合は実際より高めに出る (退出の記録は #125 で見送り済み)
//! - `joined_at` は #125 (2026-08-30) より前の参加分は運営ログからの backfill による推定値で、
//!   backfill できなかったギルドは NULL (どの月にも入れず、[`GuildRetention::unknown_joined`] で件数だけ出す)
//! - 削除された単発予定は数えられない (`event_creations` の制約)

use chrono::{Duration, NaiveDate, NaiveDateTime};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

/// 導入月 1 つぶんの定着状況
#[derive(Debug, Serialize, ToSchema, PartialEq, Eq)]
pub struct RetentionCohort {
    /// 導入月 (JST の月初)
    #[schema(example = "2026-10-01")]
    pub month: NaiveDate,
    /// その月に導入し、今も参加しているギルドの数
    #[schema(example = 300)]
    pub guilds: i64,
    /// そのうち導入から 7 日が経ったギルドの数 (7 日以内の割合の分母)
    #[schema(example = 280)]
    pub week_eligible: i64,
    /// [`RetentionCohort::week_eligible`] のうち、導入から 7 日以内に予定が作られたギルドの数
    #[schema(example = 40)]
    pub week_created: i64,
    /// そのうち導入から 30 日が経ったギルドの数 (30 日以内の割合の分母)
    #[schema(example = 200)]
    pub month_eligible: i64,
    /// [`RetentionCohort::month_eligible`] のうち、導入から 30 日以内に予定が作られたギルドの数
    #[schema(example = 50)]
    pub month_created: i64,
    /// 直近 N 日にも予定が作られているギルドの数 (分母は [`RetentionCohort::guilds`])
    #[schema(example = 20)]
    pub active: i64,
}

/// 導入月ごとの定着状況
#[derive(Debug, Serialize, ToSchema)]
pub struct GuildRetention {
    /// 導入月の新しい順。導入したギルドが無い月は含まない
    pub cohorts: Vec<RetentionCohort>,
    /// 導入日時 (`joined_at`) が分からない参加中のギルドの数 (どの月にも入れていない)
    #[schema(example = 12)]
    pub unknown_joined: i64,
    /// [`GuildRetention::unknown_joined`] のうち、直近 N 日に予定が作られたギルドの数
    #[schema(example = 1)]
    pub unknown_joined_active: i64,
}

/// 導入月ごとの定着状況を `now` (JST) 基準で集計する。`now` より後の導入・予定は数えない
pub async fn guild_retention<'e>(
    executor: impl PgExecutor<'e>,
    now: NaiveDateTime,
    recent_days: i64,
) -> sqlx::Result<GuildRetention> {
    let recent_since = now - Duration::days(recent_days);
    let week_ago = now - Duration::days(7);
    let month_ago = now - Duration::days(30);

    // 予定の側は 1 回だけ走査する: 導入後の最初の作成日時 (first) と、直近に作ったか (recent)
    let rows = sqlx::query!(
        r#"
        WITH joined AS (
            -- joined_at が NULL (導入日が分からない) のギルドは月が NULL の 1 行にまとまる
            SELECT guild_id, joined_at FROM guilds WHERE joined_at IS NULL OR joined_at <= $1
        ),
        first AS (
            SELECT j.guild_id, min(e.created_at) AS first_at
            FROM joined j
            JOIN event_creations e
              ON e.guild_id = j.guild_id AND e.created_at >= j.joined_at AND e.created_at <= $1
            GROUP BY j.guild_id
        ),
        recent AS (
            SELECT DISTINCT guild_id FROM event_creations WHERE created_at >= $2 AND created_at <= $1
        )
        SELECT
            date_trunc('month', j.joined_at)::date AS "month?",
            count(*) AS "guilds!",
            count(*) FILTER (WHERE j.joined_at <= $3) AS "week_eligible!",
            count(*) FILTER (
                WHERE j.joined_at <= $3 AND f.first_at <= j.joined_at + interval '7 days'
            ) AS "week_created!",
            count(*) FILTER (WHERE j.joined_at <= $4) AS "month_eligible!",
            count(*) FILTER (
                WHERE j.joined_at <= $4 AND f.first_at <= j.joined_at + interval '30 days'
            ) AS "month_created!",
            count(r.guild_id) AS "active!"
        FROM joined j
        LEFT JOIN first f ON f.guild_id = j.guild_id
        LEFT JOIN recent r ON r.guild_id = j.guild_id
        GROUP BY 1
        ORDER BY 1 DESC NULLS LAST
        "#,
        now,
        recent_since,
        week_ago,
        month_ago,
    )
    .fetch_all(executor)
    .await?;

    let mut retention = GuildRetention {
        cohorts: Vec::with_capacity(rows.len()),
        unknown_joined: 0,
        unknown_joined_active: 0,
    };
    for row in rows {
        match row.month {
            Some(month) => retention.cohorts.push(RetentionCohort {
                month,
                guilds: row.guilds,
                week_eligible: row.week_eligible,
                week_created: row.week_created,
                month_eligible: row.month_eligible,
                month_created: row.month_created,
                active: row.active,
            }),
            None => {
                retention.unknown_joined = row.guilds;
                retention.unknown_joined_active = row.active;
            }
        }
    }
    Ok(retention)
}
