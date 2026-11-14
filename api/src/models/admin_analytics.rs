//! 管理コンソールの分析情報 (`GET /admin/analytics`、#79) の集計。
//!
//! 概要 (`admin_stats`) が「今この瞬間の件数」なのに対して、ここは**時間軸の指標**を出す。
//!
//! # 何を「利用」とみなすか
//!
//! - **アクティブユーザー**: `user_daily_activity` (api が認証したリクエストを 1 人 1 日 1 行で積み上げた記録、#81。
//!   [`crate::models::user_activity`]) から数える。記録を入れた日 ([`MeasuredActiveUsers::since`]) からしか無い
//! - **新規ユーザー**: `user.createdAt`
//! - **予定の作成**: 単発の `events.created_at` と初期シリーズの `event_series.created_at` (JST)。分割先・補充した回は除外
//! - **予定の作成元**: `event_history` の `create` の行の `source` ([`creation_sources`])
//!
//! かつては Better Auth の `session` の生存期間からアクティブユーザーとログイン回数を推定していたが、
//! 期限切れセッションの定期削除 (#44) で行が最後の利用から約 7 日で消えるようになり、過去の期間が
//! さかのぼって減るので使っていない (#398)。**分析に `session` を使わない**
//!
//! # 精度の限界 (画面にも注記する)
//!
//! - 予定はゴミ箱 (#159) に入っている間は数え、30 日後に完全に消えると数えられなくなる。
//!   繰り返しは初回の削除後もシリーズから数えるが、シリーズ自体の全削除では減る
//! - `guilds` に参加・退出の日時が無いため、ギルド数の推移は出せない (`admin_stats` と同じ制約)
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
//! 日付の区切りはすべて JST。`user` / `event_history` は `TIMESTAMPTZ` なので SQL 側で
//! `AT TIME ZONE 'Asia/Tokyo'` を明示し、DB の `TimeZone` 設定に依存しないようにする。

use chrono::{Duration, NaiveDate, NaiveDateTime};
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

/// アクティブユーザー数 (DAU / WAU / MAU)。`user_daily_activity` (#81) から数え、JST の日付で区切る
#[derive(Debug, Serialize, ToSchema)]
pub struct ActiveUsers {
    /// 記録が始まった日 (一番古い行の日付)。まだ 1 行も無ければ null。
    /// これより前の期間は数えられない
    #[schema(example = "2026-08-28")]
    pub since: Option<NaiveDate>,
    /// 今日 (JST。まだ途中)。前は昨日
    pub daily: Trend,
    /// 今日までの 7 日間。前はその前の 7 日間
    pub weekly: Trend,
    /// 今日までの 30 日間。前はその前の 30 日間
    pub monthly: Trend,
    /// DAU/MAU 比 (stickiness、0〜1)。**昨日までの [`RECENT_DAYS`] 日間** (今日は途中なので除く) の
    /// 日ごとの利用者数の平均を、同じ期間に 1 回でも使った人数で割ったもの。
    /// 1 に近いほど同じ人が毎日使っている。その期間に利用が無い、または記録の開始がその期間の途中で
    /// 平均が薄まる場合は null
    #[schema(example = 0.18)]
    pub stickiness: Option<f64>,
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
    /// その日に使った利用者 (#81)。記録開始 ([`ActiveUsers::since`]) 前の日は 0
    pub active_users: i64,
}

/// 月別の 1 点 (JST の月初)
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct MonthlyPoint {
    #[schema(example = "2026-08-01")]
    pub month: NaiveDate,
    /// その月 (暦月) に 1 回でも使った利用者 (#81)。記録開始 ([`ActiveUsers::since`]) より前の月は 0、
    /// 記録開始の月は途中からの値
    pub active_users: i64,
    pub new_users: i64,
    pub events: i64,
}

/// 予定の作成元ごとの件数 (`event_history` の `create`)
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct CreationSource {
    /// `web` (画面・API。iCalendar の取り込みを含む) / `bot` (スラッシュコマンド) /
    /// `admin` (管理コンソール) / `mcp` (AI アシスタント)
    #[schema(example = "web")]
    pub source: String,
    /// 対象期間に作られた予定の数
    pub count: i64,
}

/// 予定の作成数（補充した回は除外）
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
    /// 開始時刻の通知に**追加で**通知が飛ぶ予定の数。
    /// 解釈できない設定と、発火時刻が重なる設定 (「60 分前」と「1 時間前」、保存済みの「0 分前」) は
    /// Bot がまとめて 1 回にするので数に入れない
    pub events_with_notifications: i64,
    /// 予定 1 件あたりの追加の通知数の平均。予定が 0 件なら 0
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

/// DAU / WAU / MAU と DAU/MAU 比 (#81)。`today` は JST の今日 (今日はまだ途中)。
///
/// 期間は JST の日付で区切る (記録が日単位なので)。`today` より未来の日付の行は、
/// 他の集計と同じ考え方でどの期間にも入れない (時計のずれや手動投入で入りうる)
pub async fn active_users<'e>(
    executor: impl PgExecutor<'e>,
    today: NaiveDate,
) -> sqlx::Result<ActiveUsers> {
    let row = sqlx::query!(
        r#"
        SELECT
            min(day) AS "since",
            count(DISTINCT user_id) FILTER (WHERE day = $1) AS "day!",
            count(DISTINCT user_id) FILTER (WHERE day = $1::date - 1) AS "day_prev!",
            count(DISTINCT user_id) FILTER (WHERE day > $1::date - 7) AS "week!",
            count(DISTINCT user_id) FILTER (
                WHERE day > $1::date - 14 AND day <= $1::date - 7) AS "week_prev!",
            count(DISTINCT user_id) FILTER (WHERE day > $1::date - 30) AS "month!",
            count(DISTINCT user_id) FILTER (
                WHERE day > $1::date - 60 AND day <= $1::date - 30) AS "month_prev!",
            -- DAU/MAU 比の材料。昨日までの $2 日間。主キーが (user_id, day) なので
            -- count(*) が「日ごとの利用者数の合計」になる
            count(*) FILTER (
                WHERE day >= $1::date - $2::int AND day < $1) AS "stickiness_days!",
            count(DISTINCT user_id) FILTER (
                WHERE day >= $1::date - $2::int AND day < $1) AS "stickiness_users!"
        FROM user_daily_activity
        WHERE day <= $1
        "#,
        today,
        RECENT_DAYS as i32,
    )
    .fetch_one(executor)
    .await?;

    Ok(ActiveUsers {
        since: row.since,
        daily: Trend::new(row.day, row.day_prev),
        weekly: Trend::new(row.week, row.week_prev),
        monthly: Trend::new(row.month, row.month_prev),
        stickiness: stickiness(row.since, today, row.stickiness_days, row.stickiness_users),
    })
}

/// DAU/MAU 比。`user_days` は昨日までの [`RECENT_DAYS`] 日間の (利用者, 日) の組の数、
/// `users` は同じ期間の利用者数。記録の開始がその期間より後なら、記録の無い日が 0 人として
/// 平均に入って低く出るので出さない
fn stickiness(
    since: Option<NaiveDate>,
    today: NaiveDate,
    user_days: i64,
    users: i64,
) -> Option<f64> {
    let window_start = today - Duration::days(RECENT_DAYS);
    if users == 0 || since.is_none_or(|since| since > window_start) {
        return None;
    }
    Some(user_days as f64 / RECENT_DAYS as f64 / users as f64)
}

/// 日別の推移 ([`DAILY_DAYS`] 日ぶん、古い順)。`today` は JST の今日。
///
/// 各テーブルを期間で絞って 1 回だけ集計し、生成した日付の列に突き合わせる
/// (日ごとの相関サブクエリにすると `events` を日数ぶん走査してしまう)
pub async fn daily<'e>(
    executor: impl PgExecutor<'e>,
    now: NaiveDateTime,
) -> sqlx::Result<Vec<DailyPoint>> {
    sqlx::query_as!(
        DailyPoint,
        r#"
        WITH days AS (
            SELECT ($1::timestamp::date - g)::date AS day
            FROM generate_series($2::int - 1, 0, -1) AS g
        ),
        -- 右端 (今日) は $1 (集計時刻) で切る。翌 0 時まで読むと、スナップショット確定までに
        -- 作られた行や未来日時の行が入り、同じ上限を持つ直近 24 時間などの件数と食い違う
        ev AS (
            SELECT created_at::date AS day, count(*) AS n
            FROM event_creations
            WHERE created_at >= ($1::timestamp::date - ($2::int - 1))::timestamp
              AND created_at <= $1
            GROUP BY 1
        ),
        nu AS (
            SELECT ("createdAt" AT TIME ZONE 'Asia/Tokyo')::date AS day, count(*) AS n
            FROM "user"
            WHERE "createdAt" >= ($1::timestamp::date - ($2::int - 1))::timestamp AT TIME ZONE 'Asia/Tokyo'
              AND "createdAt" <= $1 AT TIME ZONE 'Asia/Tokyo'
            GROUP BY 1
        ),
        -- アクティブユーザー (#81)。主キーが (user_id, day) なので count(*) が人数になる。
        -- 右端 (今日) は日付単位の記録なので、集計時刻ではなく日付そのもので切る
        au AS (
            SELECT day, count(*) AS n
            FROM user_daily_activity
            WHERE day >= ($1::timestamp::date - ($2::int - 1))
              AND day <= $1::timestamp::date
            GROUP BY 1
        )
        SELECT
            d.day AS "date!",
            COALESCE(ev.n, 0) AS "events!",
            COALESCE(nu.n, 0) AS "new_users!",
            COALESCE(au.n, 0) AS "active_users!"
        FROM days d
        LEFT JOIN ev ON ev.day = d.day
        LEFT JOIN nu ON nu.day = d.day
        LEFT JOIN au ON au.day = d.day
        ORDER BY d.day
        "#,
        now,
        DAILY_DAYS,
    )
    .fetch_all(executor)
    .await
}

/// 月別の推移 ([`MONTHLY_MONTHS`] ヶ月ぶん、古い順)。`now` は集計の基準時刻 (JST)。
///
/// 月の区切りは JST。`active_users` はその暦月に 1 回でも使った利用者の数 (`user_daily_activity`)
pub async fn monthly<'e>(
    executor: impl PgExecutor<'e>,
    now: NaiveDateTime,
) -> sqlx::Result<Vec<MonthlyPoint>> {
    sqlx::query_as!(
        MonthlyPoint,
        r#"
        WITH months AS (
            SELECT date_trunc('month', $1::timestamp) - make_interval(months => g) AS month_start
            FROM generate_series($2::int - 1, 0, -1) AS g
        ),
        span AS (
            SELECT min(month_start) AS lo FROM months
        ),
        -- 右端 (当月) は $1 (集計時刻) で切る。翌月頭まで読むと、スナップショット確定までに
        -- 作られた行や未来日時の行が入り、同じ上限を持つ直近 30 日などの件数と食い違う
        ev AS (
            SELECT date_trunc('month', e.created_at) AS month_start, count(*) AS n
            FROM event_creations e, span
            WHERE e.created_at >= span.lo AND e.created_at <= $1
            GROUP BY 1
        ),
        nu AS (
            SELECT date_trunc('month', u."createdAt" AT TIME ZONE 'Asia/Tokyo') AS month_start,
                   count(*) AS n
            FROM "user" u, span
            WHERE u."createdAt" >= span.lo AT TIME ZONE 'Asia/Tokyo'
              AND u."createdAt" <= $1 AT TIME ZONE 'Asia/Tokyo'
            GROUP BY 1
        ),
        -- 右端は日付単位の記録なので、日別と同じく今日の日付そのもので切る
        au AS (
            SELECT date_trunc('month', a.day) AS month_start, count(DISTINCT a.user_id) AS n
            FROM user_daily_activity a, span
            WHERE a.day >= span.lo::date AND a.day <= $1::timestamp::date
            GROUP BY 1
        )
        SELECT
            m.month_start::date AS "month!",
            COALESCE(au.n, 0) AS "active_users!",
            COALESCE(nu.n, 0) AS "new_users!",
            COALESCE(ev.n, 0) AS "events!"
        FROM months m
        LEFT JOIN au ON au.month_start = m.month_start
        LEFT JOIN nu ON nu.month_start = m.month_start
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

    // 直近の期間にも上限 ($7 = now) を付ける。`now` はトランザクションを開く前に取るので、
    // その後スナップショットが確定するまでに作られた予定は created_at > now で見える。
    // 時計のずれや手動投入で未来の created_at が入っている場合も同じで、
    // 上限が無いと日別の推移 (翌 0 時で切っている) と数が合わなくなる
    let row = sqlx::query!(
        r#"
        SELECT
            (SELECT count(*) FROM events WHERE deleted_at IS NULL) AS "total!",
            count(*) FILTER (WHERE created_at >= $1 AND created_at <= $7) AS "day!",
            count(*) FILTER (WHERE created_at >= $2 AND created_at < $1) AS "day_prev!",
            count(*) FILTER (WHERE created_at >= $3 AND created_at <= $7) AS "week!",
            count(*) FILTER (WHERE created_at >= $4 AND created_at < $3) AS "week_prev!",
            count(*) FILTER (WHERE created_at >= $5 AND created_at <= $7) AS "month!",
            count(*) FILTER (WHERE created_at >= $6 AND created_at < $5) AS "month_prev!",
            (SELECT count(*) FROM events WHERE is_all_day AND deleted_at IS NULL) AS "all_day!"
        FROM event_creations
        "#,
        day,
        day_prev,
        week,
        week_prev,
        month,
        month_prev,
        now,
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

/// 通知設定の集計 (実際に通知が増える予定の数, 増える通知の総数)。
///
/// 数えるのは **Bot が実際に送る通知のうち、必ず送られる開始時刻の通知を除いたもの**。
/// [`Notification::fire_times`] が Bot と同じ規則で並べてくれるので、その件数から 1
/// (= 開始時刻の通知。必ず 1 件含まれる) を引く。つまり
///
/// - 解釈できない設定 (壊れた JSON・負の `num`・未知の単位) は数えない
/// - 「60 分前」と「1 時間前」のように発火時刻が同じものは 1 件として数える
/// - 保存済みの「0 分前」は開始時刻の通知と同じなので数えない
/// - `num` が大きすぎて発火時刻を計算できないものは Bot も送れないので数えない
///
/// `notifications` の要素の中身は DB では検証していない (CHECK 制約は JSONB が配列であることしか
/// 見ていない) ので、判定を SQL に書き写すと本体 (`models::notifications`) とずれる。
/// 設定が入っている予定だけを取り出して同じ関数に通す (行はストリームで受けて溜め込まない)
pub async fn notification_stats<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<(i64, i64)> {
    let mut rows = sqlx::query!(
        r#"
        SELECT start_at, is_all_day, notifications
        FROM events WHERE jsonb_array_length(notifications) > 0 AND deleted_at IS NULL
        "#
    )
    .fetch(executor);

    let mut events_with_notifications = 0;
    let mut total = 0;
    while let Some(row) = rows.try_next().await? {
        // fire_times には必ず開始時刻の通知が入るので、そのぶんを引くと「設定によって増えた通知」
        // ギルド設定で開始時刻の通知を止めていても (#181)、ここでは「設定によって増えた分」を
        // 数えたいので、開始時刻の通知を含めて並べてから 1 を引く
        let extra = Notification::fire_times(row.start_at, row.is_all_day, &row.notifications, true)
            .len() as i64
            - 1;
        if extra > 0 {
            events_with_notifications += 1;
            total += extra;
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
    now: NaiveDateTime,
) -> sqlx::Result<(i64, i64, i64)> {
    let row = sqlx::query!(
        r#"
        WITH recent AS (
            -- 上限を付けないと、未来の created_at を持つ予定でギルドがアクティブ扱いになる
            SELECT DISTINCT guild_id FROM event_creations WHERE created_at >= $1 AND created_at <= $2
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
        now,
    )
    .fetch_one(executor)
    .await?;
    Ok((row.active_guilds, row.active_left_guilds, row.joined_guilds))
}

/// `since` より後の予定の作成数が多いギルド ([`TOP_GUILD_LIMIT`] 件まで)
pub async fn top_guilds<'e>(
    executor: impl PgExecutor<'e>,
    since: NaiveDateTime,
    now: NaiveDateTime,
) -> sqlx::Result<Vec<TopGuild>> {
    sqlx::query_as!(
        TopGuild,
        r#"
        WITH recent AS (
            SELECT guild_id, count(*) AS n
            FROM event_creations WHERE created_at >= $1 AND created_at <= $2 GROUP BY guild_id
        )
        SELECT
            r.guild_id AS "guild_id!",
            g.name AS "name?",
            g.avatar_url AS "avatar_url?",
            r.n AS "event_count!"
        FROM recent r
        LEFT JOIN guilds g ON g.guild_id = r.guild_id
        ORDER BY r.n DESC, r.guild_id
        LIMIT $3
        "#,
        since,
        now,
        TOP_GUILD_LIMIT,
    )
    .fetch_all(executor)
    .await
}

/// `since` より後に作られた予定を作成元 (`event_history.source`) ごとに数える。多い順。
///
/// 予定の作成は 1 件につき `create` の履歴が 1 行入る (繰り返しは最初の回にだけ入り、補充した回には入らない)
/// ので、件数は [`EventCreation`] とおおむね揃う。ただし次の分だけずれる (画面にも注記する)
///
/// - 履歴は 180 日で消え、ゴミ箱から完全に消えた予定の履歴も一緒に消える (`ON DELETE CASCADE`)
/// - 予定ごとの上限 (`event_history::MAX_PER_EVENT`) を超えて更新された予定は、古い `create` の行から消える
/// - 履歴の記録 (#165) より前に作られた予定は数えられない
///
/// iCalendar の取り込み (`routes::imports`) は画面からの操作なので `web` に入る
pub async fn creation_sources<'e>(
    executor: impl PgExecutor<'e>,
    since: NaiveDateTime,
    now: NaiveDateTime,
) -> sqlx::Result<Vec<CreationSource>> {
    sqlx::query_as!(
        CreationSource,
        r#"
        SELECT source AS "source!", count(*) AS "count!"
        FROM event_history
        WHERE action = 'create'
          AND created_at >= $1::timestamp AT TIME ZONE 'Asia/Tokyo'
          AND created_at <= $2::timestamp AT TIME ZONE 'Asia/Tokyo'
        GROUP BY source
        ORDER BY count(*) DESC, source
        "#,
        since,
        now,
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
        WITH channels AS (
            -- 旧スキーマには guild_id の一意制約も索引も無いので、ギルドごとの相関サブクエリにすると
            -- event_settings をギルドの数だけ走査してしまう。先頭の 1 行を一度に取って結合する
            -- (先頭を使うのは Bot / admin_stats と同じ)
            SELECT DISTINCT ON (guild_id) guild_id, channel_id
            FROM event_settings ORDER BY guild_id, id
        )
        SELECT
            g.guild_id,
            c.channel_id,
            COALESCE(gc.restricted, false) AS "restricted!"
        FROM guilds g
        LEFT JOIN channels c ON c.guild_id = g.guild_id
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
    use chrono::NaiveDate;

    use super::{Trend, percent_change, stickiness};

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

    fn date(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn stickiness_is_the_average_dau_over_mau() {
        // 30 日間に 10 人。延べ 60 (人・日) なら 1 日平均 2 人 = 0.2
        let today = date("2026-11-15");
        let ratio = stickiness(Some(date("2026-08-28")), today, 60, 10).unwrap();
        assert!((ratio - 0.2).abs() < 1e-9);
        // 記録開始がちょうど期間の初日なら出す
        assert!(stickiness(Some(date("2026-10-16")), today, 60, 10).is_some());
    }

    #[test]
    fn stickiness_is_unknown_without_a_full_window() {
        let today = date("2026-11-15");
        // 利用が無い
        assert_eq!(stickiness(Some(date("2026-08-28")), today, 0, 0), None);
        assert_eq!(stickiness(None, today, 0, 0), None);
        // 記録の開始が期間の途中だと、記録の無い日が 0 人として平均を下げてしまう
        assert_eq!(stickiness(Some(date("2026-10-17")), today, 60, 10), None);
    }
}
