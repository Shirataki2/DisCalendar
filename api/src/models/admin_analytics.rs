//! 管理コンソールの分析情報 (`GET /admin/analytics`、#79) の集計。
//!
//! 概要 (`admin_stats`) が「今この瞬間の件数」なのに対して、ここは**時間軸の指標**を出す。
//! DisCalendar には行動ログのテーブルが無いので、集計は既存のテーブルから読み取れる痕跡だけで行う。
//! スキーマは変更しない (AGENTS.md の P0)。正確な指標を取る仕組みは #81 で検討する。
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
//! - **セッションの `updatedAt` を更新するのは Better Auth (web のページ表示) だけ**で、
//!   api の認証 (`crate::auth`) は `session` を読むだけで更新しない。そのためダッシュボードを開いたまま
//!   予定の作成・カレンダーの再取得だけを続けている利用者は、最初の表示から 24 時間を過ぎると
//!   アクティブから漏れる。ここを正確にするには行動の記録が要る (#81)
//! - `guilds` に参加・退出の日時が無いため、ギルド数の推移は出せない (`admin_stats` と同じ制約)
//! - `updatedAt` の更新間隔は最短 1 日なので、日単位より細かい粒度は出せない
//! - 推移の右端 (今日 / 当月) は期間の途中までしか含まない。画面ではその旨を注記する
//!
//! # 走査の回数について
//!
//! 日別・月別は**日 (月) ごとの相関サブクエリにしない**。`events.created_at` に索引が無いので、
//! そのまま書くと 1 回のリクエストで `events` を 30 + 12 回走査してしまう。
//! 期間を絞って 1 回だけ集計し、生成した日付・月の列と突き合わせる形にしてある。
//! 呼び出し側 (`routes::admin_analytics`) は読み取り専用の REPEATABLE READ トランザクション 1 本で
//! 順に実行する。既定 5 本のプールを占有して通常の API を待たせないためと、
//! 集計の途中で予定が作られても数字どうしが食い違わない (割合が 100% を超えるなど) ようにするため。
//!
//! 日付の区切りはすべて JST。`session` / `user` は `TIMESTAMPTZ` なので SQL 側で
//! `AT TIME ZONE 'Asia/Tokyo'` を明示し、DB の `TimeZone` 設定に依存しないようにする。

use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use futures_util::TryStreamExt;
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use super::{admin_stats::is_sendable_channel, notifications::Notification};

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
    /// 実際に通知される設定を 1 件以上持つ予定の数
    /// (Bot が必ず送る開始時刻の通知は含めない。解釈できない設定も含めない)
    pub events_with_notifications: i64,
    /// 予定 1 件あたりの通知設定数の平均。予定が 0 件なら 0
    #[schema(example = 1.4)]
    pub notifications_per_event: f64,
    /// 参加中のギルドのうち、通知先チャンネルに**形式として正しい** ID を設定しているものの数。
    /// そのチャンネルが今も存在するか・Bot に送信権限があるかまでは確認していない
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
    /// 直近 [`RECENT_DAYS`] 日に予定が作られた**参加中**のギルドの数。
    /// [`GuildActivity::joined_guilds`] との割合を出せるよう、退出済みは含めない
    pub active_guilds: i64,
    /// 直近 [`RECENT_DAYS`] 日に予定が作られたが `guilds` に行が無いギルドの数
    /// (退出済みのギルドに残っている予定。割合の分子には入れない)
    pub active_left_guilds: i64,
    /// `guilds` テーブルの行数 (Bot が参加中のギルド。割合の分母)
    pub joined_guilds: i64,
    /// 直近 [`RECENT_DAYS`] 日の予定作成数が多いギルド (退出済みも含む)
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

/// 日別の推移 ([`DAILY_DAYS`] 日ぶん、古い順)。`today` は JST の今日。
///
/// 各テーブルを期間で絞って 1 回だけ集計し、生成した日付の列に突き合わせる
/// (日ごとの相関サブクエリにすると `events` を日数ぶん走査してしまう)
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
        ),
        ev AS (
            SELECT created_at::date AS day, count(*) AS n
            FROM events
            WHERE created_at >= ($1::date - ($2::int - 1))::timestamp
              AND created_at < ($1::date + 1)::timestamp
            GROUP BY 1
        ),
        nu AS (
            SELECT ("createdAt" AT TIME ZONE 'Asia/Tokyo')::date AS day, count(*) AS n
            FROM "user"
            WHERE "createdAt" >= ($1::date - ($2::int - 1))::timestamp AT TIME ZONE 'Asia/Tokyo'
              AND "createdAt" < ($1::date + 1)::timestamp AT TIME ZONE 'Asia/Tokyo'
            GROUP BY 1
        ),
        lg AS (
            SELECT ("createdAt" AT TIME ZONE 'Asia/Tokyo')::date AS day, count(*) AS n
            FROM "session"
            WHERE "createdAt" >= ($1::date - ($2::int - 1))::timestamp AT TIME ZONE 'Asia/Tokyo'
              AND "createdAt" < ($1::date + 1)::timestamp AT TIME ZONE 'Asia/Tokyo'
            GROUP BY 1
        )
        SELECT
            d.day AS "date!",
            COALESCE(ev.n, 0) AS "events!",
            COALESCE(nu.n, 0) AS "new_users!",
            COALESCE(lg.n, 0) AS "logins!"
        FROM days d
        LEFT JOIN ev ON ev.day = d.day
        LEFT JOIN nu ON nu.day = d.day
        LEFT JOIN lg ON lg.day = d.day
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
/// 期限切れセッションを削除するとさかのぼって減る。
/// 生存期間の重なりは月ごとの集計にできないので `months` との結合で数えるが、
/// `session` の走査は 1 回で済ませる
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
        ),
        span AS (
            SELECT min(month_start) AS lo, max(month_start) + interval '1 month' AS hi
            FROM months
        ),
        ev AS (
            SELECT date_trunc('month', e.created_at) AS month_start, count(*) AS n
            FROM events e, span
            WHERE e.created_at >= span.lo AND e.created_at < span.hi
            GROUP BY 1
        ),
        nu AS (
            SELECT date_trunc('month', u."createdAt" AT TIME ZONE 'Asia/Tokyo') AS month_start,
                   count(*) AS n
            FROM "user" u, span
            WHERE u."createdAt" >= span.lo AT TIME ZONE 'Asia/Tokyo'
              AND u."createdAt" < span.hi AT TIME ZONE 'Asia/Tokyo'
            GROUP BY 1
        ),
        lg AS (
            SELECT date_trunc('month', s."createdAt" AT TIME ZONE 'Asia/Tokyo') AS month_start,
                   count(*) AS n
            FROM "session" s, span
            WHERE s."createdAt" >= span.lo AT TIME ZONE 'Asia/Tokyo'
              AND s."createdAt" < span.hi AT TIME ZONE 'Asia/Tokyo'
            GROUP BY 1
        ),
        au AS (
            SELECT m.month_start, count(DISTINCT s."userId") AS n
            FROM months m
            LEFT JOIN "session" s
              ON s."createdAt" < (m.month_start + interval '1 month') AT TIME ZONE 'Asia/Tokyo'
             AND s."updatedAt" >= m.month_start AT TIME ZONE 'Asia/Tokyo'
            GROUP BY m.month_start
        )
        SELECT
            m.month_start::date AS "month!",
            COALESCE(au.n, 0) AS "active_users!",
            COALESCE(nu.n, 0) AS "new_users!",
            COALESCE(lg.n, 0) AS "logins!",
            COALESCE(ev.n, 0) AS "events!"
        FROM months m
        LEFT JOIN au ON au.month_start = m.month_start
        LEFT JOIN nu ON nu.month_start = m.month_start
        LEFT JOIN lg ON lg.month_start = m.month_start
        LEFT JOIN ev ON ev.month_start = m.month_start
        ORDER BY m.month_start
        "#,
        now,
        MONTHLY_MONTHS,
    )
    .fetch_all(executor)
    .await
}

/// 予定の作成数と終日予定の数。`now` は JST の現在時刻
pub async fn event_creation<'e>(
    executor: impl PgExecutor<'e>,
    now: NaiveDateTime,
) -> sqlx::Result<(EventCreation, i64)> {
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
            count(*) FILTER (WHERE is_all_day) AS "all_day!"
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

    Ok((
        EventCreation {
            last_day: Trend::new(row.day, row.day_prev),
            last_week: Trend::new(row.week, row.week_prev),
            last_month: Trend::new(row.month, row.month_prev),
            total: row.total,
        },
        row.all_day,
    ))
}

/// 通知設定の集計 (実際に通知される設定を持つ予定の数, 通知設定の総数)。
///
/// `notifications` は制約のない `TEXT[]` で、旧データや手動修正で壊れた JSON・負の `num`・
/// 未知の単位が入りうる。api も Bot も [`Notification::decode_all`] で**解釈できない要素を捨てる**ので、
/// 配列の長さをそのまま数えると「実際には通知されない予定」を数に入れてしまう。
/// 判定を SQL に書き写すと本体 (`models::notifications`) とずれるため、
/// 設定が入っている予定だけを取り出して同じ関数で数える (行はストリームで受けて溜め込まない)
pub async fn notification_stats<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<(i64, i64)> {
    let mut rows = sqlx::query!(
        r#"SELECT notifications FROM events WHERE array_length(notifications, 1) > 0"#
    )
    .fetch(executor);

    let mut events_with_notifications = 0;
    let mut total = 0;
    while let Some(row) = rows.try_next().await? {
        let valid = Notification::decode_all(&row.notifications).len() as i64;
        if valid > 0 {
            events_with_notifications += 1;
            total += valid;
        }
    }
    Ok((events_with_notifications, total))
}

/// 直近に予定が作られたギルドの数を (参加中, 退出済み, 参加中の総数) で返す。
/// `since` より後に作られた予定を「アクティブ」の根拠にする (JST)。
///
/// 退出済み (`guilds` に行が無いのに予定が残っている) を分けるのは、
/// 混ぜると「参加中のギルドのうちどれだけ使われているか」の割合が出せないため
/// (分子に退出済みが入り、分母より大きくなることすらある)
pub async fn guild_counts<'e>(
    executor: impl PgExecutor<'e>,
    since: NaiveDateTime,
) -> sqlx::Result<(i64, i64, i64)> {
    let row = sqlx::query!(
        r#"
        WITH recent AS (
            SELECT DISTINCT guild_id FROM events WHERE created_at >= $1
        )
        SELECT
            -- 参加中と退出済みを分けて数える。混ぜると「参加中のうち使われている割合」が出せない
            count(*) FILTER (WHERE g.guild_id IS NOT NULL) AS "active_guilds!",
            count(*) FILTER (WHERE g.guild_id IS NULL) AS "active_left_guilds!",
            (SELECT count(*) FROM guilds) AS "joined_guilds!"
        FROM recent r
        LEFT JOIN guilds g ON g.guild_id = r.guild_id
        "#,
        since,
    )
    .fetch_one(executor)
    .await?;
    Ok((row.active_guilds, row.active_left_guilds, row.joined_guilds))
}

/// `since` より後の予定の作成数が多いギルド ([`TOP_GUILD_LIMIT`] 件まで)
pub async fn top_guilds<'e>(
    executor: impl PgExecutor<'e>,
    since: NaiveDateTime,
) -> sqlx::Result<Vec<TopGuild>> {
    sqlx::query_as!(
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
    .await
}

/// 参加中のギルドの設定状況 (通知先チャンネルの ID が正しい形式の数, restricted の数)。
///
/// 通知先は「行があること」ではなく `channel_id` が **Snowflake として正しい形式か** で数える
/// (旧データの `"0"` などが混ざっているため。判定は `admin_stats` と共通)。
/// そのチャンネルが今も存在するか・Bot に送信権限があるかは Discord に聞かないと分からないので、
/// ここで分かるのは「設定として成立しているか」までにとどまる
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
