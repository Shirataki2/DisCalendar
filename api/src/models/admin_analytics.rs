//! 管理コンソールの分析情報 (`GET /admin/analytics`、#79) の集計。
//!
//! 概要 (`admin_stats`) が「今この瞬間の件数」なのに対して、ここは**時間軸の指標**を出す。
//! DisCalendar には行動ログのテーブルが無いので、集計は既存のテーブルから読み取れる痕跡だけで行う。
//! スキーマは変更しない (AGENTS.md の P0)。
//!
//! # 何を「利用」とみなすか
//!
//! - **アクティブユーザー**: Better Auth の `session` を使う。セッションは既定で 7 日使われないと失効し、
//!   使われている間は `updateAge` (既定 1 日) ごとに `expiresAt` が延長されて `updatedAt` が更新される。
//!   つまり 1 行の生存期間 `[createdAt, updatedAt]` は「その利用者がログインしてから最後に使うまで」に相当する。
//!   ある期間に**その区間が重なる**ユーザーを、その期間のアクティブユーザーとして数える
//!   (`updatedAt` が期間内にあるかどうかで数えると、今も使い続けている利用者の行は最新の時刻に動いてしまい、
//!   過去の期間が実際より少なく出る)
//! - **ログイン**: `session` の行が作られた回数 (`createdAt`)
//! - **新規ユーザー**: `user.createdAt`
//! - **予定の作成**: `events.created_at` (api / Bot が `now_jst()` で入れるタイムゾーンなしの JST)
//!
//! # 精度の限界 (画面にも注記する)
//!
//! - 削除されたデータは数えられない。予定は削除で行ごと消えるので、過去の作成数は実際より少なく出る
//! - 期限切れセッションを消す (定型操作 `purge-expired-sessions` / #44) と、その分だけ過去の
//!   アクティブユーザーが減る。掃除を実行した時期より前の値は信用できない
//! - `guilds` に参加・退出の日時が無いため、ギルド数の推移は出せない (`admin_stats` と同じ制約)
//! - `updatedAt` の更新間隔は最短 1 日なので、日単位より細かい粒度は出せない
//!
//! 日付の区切りはすべて JST。`session` / `user` は `TIMESTAMPTZ` なので SQL 側で
//! `AT TIME ZONE 'Asia/Tokyo'` を明示し、DB の `TimeZone` 設定に依存しないようにする。

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use super::admin_stats::is_sendable_channel;

/// 日別の推移で見る日数
pub const DAILY_DAYS: i32 = 30;
/// 月別の推移で見る月数
pub const MONTHLY_MONTHS: i32 = 12;
/// 予定の作成数が多いギルドを何件まで出すか
pub const TOP_GUILD_LIMIT: i64 = 10;
/// 「アクティブ」「直近」の基準にする日数 (MAU と同じ窓)
pub const RECENT_DAYS: i64 = 30;

/// ある期間の件数と、その直前の同じ長さの期間との比較
#[derive(Debug, Serialize, ToSchema)]
pub struct Trend {
    /// 対象期間の値
    #[schema(example = 120)]
    pub current: i64,
    /// 直前の同じ長さの期間の値
    #[schema(example = 100)]
    pub previous: i64,
    /// `current - previous`
    #[schema(example = 20)]
    pub delta: i64,
    /// 増減率 (%)。`previous` が 0 のときは計算できないので null
    #[schema(example = 20.0)]
    pub change_percent: Option<f64>,
}

impl Trend {
    fn new(current: i64, previous: i64) -> Self {
        Self {
            current,
            previous,
            delta: current - previous,
            change_percent: percent_change(current, previous),
        }
    }
}

/// 増減率 (%)。分母が 0 なら計算できない (「0 から 1 に増えた」は ∞% で意味を持たない)
fn percent_change(current: i64, previous: i64) -> Option<f64> {
    if previous == 0 {
        return None;
    }
    Some((current - previous) as f64 / previous as f64 * 100.0)
}

/// アクティブユーザー数 (DAU / WAU / MAU)
#[derive(Debug, Serialize, ToSchema)]
pub struct ActiveUsers {
    /// 直近 1 日 (DAU)
    pub daily: Trend,
    /// 直近 7 日 (WAU)
    pub weekly: Trend,
    /// 直近 30 日 (MAU)
    pub monthly: Trend,
}

/// 日別の 1 点 (JST の日付)
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct DailyPoint {
    #[schema(example = "2026-08-25")]
    pub date: NaiveDate,
    /// その日に作られた予定
    pub events: i64,
    /// その日に登録された利用者
    pub new_users: i64,
    /// その日に作られたセッション (ログイン)
    pub logins: i64,
}

/// 月別の 1 点 (JST の月初)
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct MonthlyPoint {
    #[schema(example = "2026-08-01")]
    pub month: NaiveDate,
    /// その月にセッションの生存期間が重なった利用者 (その月の MAU)
    pub active_users: i64,
    pub new_users: i64,
    pub logins: i64,
    pub events: i64,
}

/// 予定の作成数
#[derive(Debug, Serialize, ToSchema)]
pub struct EventCreation {
    /// 直近 24 時間
    pub last_day: Trend,
    /// 直近 7 日
    pub last_week: Trend,
    /// 直近 30 日
    pub last_month: Trend,
    /// 今 DB にある予定の総数 (削除されたものは含まれない)
    pub total: i64,
}

/// 予定の内訳と設定の行き渡り具合
#[derive(Debug, Serialize, ToSchema)]
pub struct Breakdown {
    /// 終日予定の数
    pub all_day_events: i64,
    /// 通知を 1 件以上設定している予定の数 (Bot が必ず送る開始時刻の通知は含めない)
    pub events_with_notifications: i64,
    /// 予定 1 件あたりの通知設定数の平均。予定が 0 件なら 0
    #[schema(example = 1.4)]
    pub notifications_per_event: f64,
    /// 参加中のギルドのうち、Bot が送信できる通知先チャンネルを設定しているものの数
    pub guilds_with_channel: i64,
    /// 参加中のギルドのうち restricted モードのものの数
    pub restricted_guilds: i64,
}

/// 予定の作成数が多いギルド
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct TopGuild {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    /// `guilds` にある名前。退出済みで行が無ければ null
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    /// 対象期間に作られた予定の数
    pub event_count: i64,
}

/// ギルドの利用状況
#[derive(Debug, Serialize, ToSchema)]
pub struct GuildActivity {
    /// 直近 [`RECENT_DAYS`] 日に予定が作られたギルドの数 (退出済みを含む)
    pub active_guilds: i64,
    /// `guilds` テーブルの行数 (Bot が参加中のギルド。割合の分母)
    pub joined_guilds: i64,
    /// 直近 [`RECENT_DAYS`] 日の予定作成数が多いギルド
    pub top_guilds: Vec<TopGuild>,
}

/// DAU / WAU / MAU。`now` は集計の基準時刻 (UTC)。
///
/// 判定はモジュールの説明のとおり「セッションの生存期間が期間に重なるか」で行う。
/// 期間 `[s, e)` に重なる = `createdAt < e AND updatedAt >= s`
pub async fn active_users<'e>(
    executor: impl PgExecutor<'e>,
    now: DateTime<Utc>,
) -> sqlx::Result<ActiveUsers> {
    let day = now - Duration::days(1);
    let day_prev = now - Duration::days(2);
    let week = now - Duration::days(7);
    let week_prev = now - Duration::days(14);
    let month = now - Duration::days(30);
    let month_prev = now - Duration::days(60);

    let row = sqlx::query!(
        r#"
        SELECT
            count(DISTINCT "userId") FILTER (
                WHERE "createdAt" < $1 AND "updatedAt" >= $2) AS "day!",
            count(DISTINCT "userId") FILTER (
                WHERE "createdAt" < $2 AND "updatedAt" >= $3) AS "day_prev!",
            count(DISTINCT "userId") FILTER (
                WHERE "createdAt" < $1 AND "updatedAt" >= $4) AS "week!",
            count(DISTINCT "userId") FILTER (
                WHERE "createdAt" < $4 AND "updatedAt" >= $5) AS "week_prev!",
            count(DISTINCT "userId") FILTER (
                WHERE "createdAt" < $1 AND "updatedAt" >= $6) AS "month!",
            count(DISTINCT "userId") FILTER (
                WHERE "createdAt" < $6 AND "updatedAt" >= $7) AS "month_prev!"
        FROM "session"
        "#,
        now,
        day,
        day_prev,
        week,
        week_prev,
        month,
        month_prev,
    )
    .fetch_one(executor)
    .await?;

    Ok(ActiveUsers {
        daily: Trend::new(row.day, row.day_prev),
        weekly: Trend::new(row.week, row.week_prev),
        monthly: Trend::new(row.month, row.month_prev),
    })
}

/// 日別の推移 ([`DAILY_DAYS`] 日ぶん、古い順)。`today` は JST の今日
pub async fn daily<'e>(
    executor: impl PgExecutor<'e>,
    today: NaiveDate,
) -> sqlx::Result<Vec<DailyPoint>> {
    sqlx::query_as!(
        DailyPoint,
        r#"
        WITH days AS (
            SELECT ($1::date - g)::date AS day
            FROM generate_series($2::int - 1, 0, -1) AS g
        )
        SELECT
            d.day AS "date!",
            (SELECT count(*) FROM events e
               WHERE e.created_at >= d.day::timestamp
                 AND e.created_at < (d.day + 1)::timestamp) AS "events!",
            (SELECT count(*) FROM "user" u
               WHERE u."createdAt" >= d.day::timestamp AT TIME ZONE 'Asia/Tokyo'
                 AND u."createdAt" < (d.day + 1)::timestamp AT TIME ZONE 'Asia/Tokyo') AS "new_users!",
            (SELECT count(*) FROM "session" s
               WHERE s."createdAt" >= d.day::timestamp AT TIME ZONE 'Asia/Tokyo'
                 AND s."createdAt" < (d.day + 1)::timestamp AT TIME ZONE 'Asia/Tokyo') AS "logins!"
        FROM days d
        ORDER BY d.day
        "#,
        today,
        DAILY_DAYS,
    )
    .fetch_all(executor)
    .await
}

/// 月別の推移 ([`MONTHLY_MONTHS`] ヶ月ぶん、古い順)。`now` は集計の基準時刻 (UTC)。
///
/// 月の区切りは JST。`active_users` はその月にセッションの生存期間が重なった利用者の数で、
/// 期限切れセッションを削除するとさかのぼって減る
pub async fn monthly<'e>(
    executor: impl PgExecutor<'e>,
    now: DateTime<Utc>,
) -> sqlx::Result<Vec<MonthlyPoint>> {
    sqlx::query_as!(
        MonthlyPoint,
        r#"
        WITH months AS (
            SELECT date_trunc('month', $1::timestamptz AT TIME ZONE 'Asia/Tokyo')
                     - make_interval(months => g) AS month_start
            FROM generate_series($2::int - 1, 0, -1) AS g
        )
        SELECT
            m.month_start::date AS "month!",
            (SELECT count(DISTINCT s."userId") FROM "session" s
               WHERE s."createdAt" < (m.month_start + interval '1 month') AT TIME ZONE 'Asia/Tokyo'
                 AND s."updatedAt" >= m.month_start AT TIME ZONE 'Asia/Tokyo') AS "active_users!",
            (SELECT count(*) FROM "user" u
               WHERE u."createdAt" >= m.month_start AT TIME ZONE 'Asia/Tokyo'
                 AND u."createdAt" < (m.month_start + interval '1 month') AT TIME ZONE 'Asia/Tokyo') AS "new_users!",
            (SELECT count(*) FROM "session" s
               WHERE s."createdAt" >= m.month_start AT TIME ZONE 'Asia/Tokyo'
                 AND s."createdAt" < (m.month_start + interval '1 month') AT TIME ZONE 'Asia/Tokyo') AS "logins!",
            (SELECT count(*) FROM events e
               WHERE e.created_at >= m.month_start
                 AND e.created_at < m.month_start + interval '1 month') AS "events!"
        FROM months m
        ORDER BY m.month_start
        "#,
        now,
        MONTHLY_MONTHS,
    )
    .fetch_all(executor)
    .await
}

/// 予定の作成数と内訳。`now` は JST の現在時刻
pub async fn events<'e>(
    executor: impl PgExecutor<'e>,
    now: NaiveDateTime,
) -> sqlx::Result<(EventCreation, i64, i64, f64)> {
    let day = now - Duration::days(1);
    let day_prev = now - Duration::days(2);
    let week = now - Duration::days(7);
    let week_prev = now - Duration::days(14);
    let month = now - Duration::days(30);
    let month_prev = now - Duration::days(60);

    let row = sqlx::query!(
        r#"
        SELECT
            count(*) AS "total!",
            count(*) FILTER (WHERE created_at >= $1) AS "day!",
            count(*) FILTER (WHERE created_at >= $2 AND created_at < $1) AS "day_prev!",
            count(*) FILTER (WHERE created_at >= $3) AS "week!",
            count(*) FILTER (WHERE created_at >= $4 AND created_at < $3) AS "week_prev!",
            count(*) FILTER (WHERE created_at >= $5) AS "month!",
            count(*) FILTER (WHERE created_at >= $6 AND created_at < $5) AS "month_prev!",
            count(*) FILTER (WHERE is_all_day) AS "all_day!",
            count(*) FILTER (WHERE array_length(notifications, 1) > 0) AS "with_notifications!",
            COALESCE(sum(COALESCE(array_length(notifications, 1), 0)), 0) AS "notification_total!"
        FROM events
        "#,
        day,
        day_prev,
        week,
        week_prev,
        month,
        month_prev,
    )
    .fetch_one(executor)
    .await?;

    let creation = EventCreation {
        last_day: Trend::new(row.day, row.day_prev),
        last_week: Trend::new(row.week, row.week_prev),
        last_month: Trend::new(row.month, row.month_prev),
        total: row.total,
    };
    let per_event = if row.total == 0 {
        0.0
    } else {
        row.notification_total as f64 / row.total as f64
    };
    Ok((creation, row.all_day, row.with_notifications, per_event))
}

/// ギルドの利用状況。`since` より後に作られた予定を「アクティブ」の根拠にする (JST)
pub async fn guild_activity<'e, E>(executor: E, since: NaiveDateTime) -> sqlx::Result<GuildActivity>
where
    E: PgExecutor<'e> + Copy,
{
    let counts = sqlx::query!(
        r#"
        SELECT
            (SELECT count(*) FROM (
                SELECT DISTINCT guild_id FROM events WHERE created_at >= $1
            ) a) AS "active_guilds!",
            (SELECT count(*) FROM guilds) AS "joined_guilds!"
        "#,
        since,
    )
    .fetch_one(executor)
    .await?;

    let top_guilds = sqlx::query_as!(
        TopGuild,
        r#"
        WITH recent AS (
            SELECT guild_id, count(*) AS n
            FROM events WHERE created_at >= $1 GROUP BY guild_id
        )
        SELECT
            r.guild_id AS "guild_id!",
            g.name AS "name?",
            g.avatar_url AS "avatar_url?",
            r.n AS "event_count!"
        FROM recent r
        LEFT JOIN guilds g ON g.guild_id = r.guild_id
        ORDER BY r.n DESC, r.guild_id
        LIMIT $2
        "#,
        since,
        TOP_GUILD_LIMIT,
    )
    .fetch_all(executor)
    .await?;

    Ok(GuildActivity {
        active_guilds: counts.active_guilds,
        joined_guilds: counts.joined_guilds,
        top_guilds,
    })
}

/// 参加中のギルドの設定状況 (通知先チャンネルを設定済みの数, restricted の数)。
///
/// 通知先は「行があること」ではなく **Bot が実際に送れる `channel_id` か** で数える
/// (旧データの `"0"` などが混ざっているため。判定は `admin_stats` と共通)
pub async fn guild_settings<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<(i64, i64)> {
    let rows = sqlx::query!(
        r#"
        SELECT
            g.guild_id,
            -- 旧スキーマには guild_id の一意制約が無いので Bot と同じく先頭の 1 行を使う
            (SELECT es.channel_id FROM event_settings es
               WHERE es.guild_id = g.guild_id ORDER BY es.id LIMIT 1) AS channel_id,
            COALESCE(gc.restricted, false) AS "restricted!"
        FROM guilds g
        LEFT JOIN guild_config gc ON gc.guild_id = g.guild_id
        "#
    )
    .fetch_all(executor)
    .await?;

    let with_channel = rows
        .iter()
        .filter(|row| row.channel_id.as_deref().is_some_and(is_sendable_channel))
        .count() as i64;
    let restricted = rows.iter().filter(|row| row.restricted).count() as i64;
    Ok((with_channel, restricted))
}

#[cfg(test)]
mod tests {
    use super::{Trend, percent_change};

    #[test]
    fn computes_the_change_against_the_previous_period() {
        let trend = Trend::new(120, 100);
        assert_eq!(trend.delta, 20);
        assert_eq!(trend.change_percent, Some(20.0));

        let trend = Trend::new(50, 100);
        assert_eq!(trend.delta, -50);
        assert_eq!(trend.change_percent, Some(-50.0));
    }

    #[test]
    fn has_no_change_rate_when_the_previous_period_is_zero() {
        // 0 → 1 の増減率は ∞ になり意味を持たないので出さない (件数と差分だけ見せる)
        let trend = Trend::new(1, 0);
        assert_eq!(trend.delta, 1);
        assert_eq!(trend.change_percent, None);
        assert_eq!(percent_change(0, 0), None);
    }
}
