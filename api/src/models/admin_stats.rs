//! 管理コンソールの概要 (`GET /admin/stats`、#37) の集計。
//!
//! 運用で最初に見る数字 (ギルド数 / 予定数 / ユーザー数 / 今日の通知予定数) と、
//! 直近のギルドの出入りをまとめて返す。
//!
//! `guilds` テーブルには作成・更新日時が無く、Bot は退出時に行を消す (bot/src/event.rs) ので、
//! 「いつ参加・退出したか」は DB からは分からない。ここで出せるのは
//! - 参加: `guilds.id` (SERIAL) の降順 = 登録された順の新しい方から
//! - 退出: `guilds` に行が無いのに予定や設定が残っているギルド (退出の痕跡)
//!
//! までで、日時は予定の `created_at` から推測するしかない。

use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::PgExecutor;
use utoipa::ToSchema;

use super::notifications::Notification;

/// 概要に載せる「直近」の件数
pub const RECENT_LIMIT: i64 = 10;

/// 今日の通知予定数を数えるときに見る予定の範囲 (今日から何日先までの予定を対象にするか)。
///
/// 通知は「開始の num unit 前」に飛ぶので、今日発火する通知を持つ予定は今日以降ならいくらでも先に
/// あり得る (`num` に上限が無いため)。全件走査を避けるため、ここで打ち切る。
/// これより先の予定に付いた「1 年以上前に通知」の設定は数に入らない
pub const NOTIFICATION_LOOKAHEAD_DAYS: i64 = 366;

/// 概要の件数
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminCounts {
    /// `guilds` テーブルの行数 (Bot が参加中として登録しているギルド)
    #[schema(example = 42)]
    pub guilds: i64,
    /// 予定・設定も含めて DB に痕跡があるギルドの数 (退出済みを含む)
    #[schema(example = 50)]
    pub known_guilds: i64,
    /// `known_guilds - guilds` (退出済みでデータだけ残っているギルド)
    #[schema(example = 8)]
    pub left_guilds: i64,
    pub events: i64,
    /// まだ終わっていない予定 (`end_at >= 現在`)
    pub upcoming_events: i64,
    /// Better Auth の `user` の行数
    pub users: i64,
    /// 期限内の `session` の行数
    pub active_sessions: i64,
    /// 期限切れを含む `session` の行数
    pub sessions: i64,
}

/// 直近に `guilds` に登録されたギルド
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct RecentGuild {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

/// `guilds` に行が無いのにデータが残っているギルド (Bot が退出した痕跡)
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct LeftGuild {
    #[schema(example = "782502586817314816")]
    pub guild_id: String,
    pub event_count: i64,
    /// 残っている予定のうち最後に作られたもの (JST)。予定が無ければ null
    #[schema(example = "2026-08-22T10:00:00")]
    pub last_event_created_at: Option<NaiveDateTime>,
}

/// 件数をまとめて 1 回のクエリで取る。`now` は「まだ終わっていない予定」の基準 (JST)
pub async fn counts<'e>(
    executor: impl PgExecutor<'e>,
    now: NaiveDateTime,
) -> sqlx::Result<AdminCounts> {
    let row = sqlx::query!(
        r#"
        SELECT
            (SELECT count(*) FROM guilds) AS "guilds!",
            (SELECT count(*) FROM (
                SELECT guild_id FROM guilds
                UNION SELECT guild_id FROM guild_config
                UNION SELECT guild_id FROM event_settings
                UNION SELECT guild_id FROM events
            ) k) AS "known_guilds!",
            (SELECT count(*) FROM events) AS "events!",
            (SELECT count(*) FROM events WHERE end_at >= $1) AS "upcoming_events!",
            (SELECT count(*) FROM "user") AS "users!",
            (SELECT count(*) FROM "session" WHERE "expiresAt" > now()) AS "active_sessions!",
            (SELECT count(*) FROM "session") AS "sessions!"
        "#,
        now
    )
    .fetch_one(executor)
    .await?;

    Ok(AdminCounts {
        guilds: row.guilds,
        known_guilds: row.known_guilds,
        left_guilds: (row.known_guilds - row.guilds).max(0),
        events: row.events,
        upcoming_events: row.upcoming_events,
        users: row.users,
        active_sessions: row.active_sessions,
        sessions: row.sessions,
    })
}

/// 直近に登録されたギルド (`guilds.id` の降順)
pub async fn recent_guilds<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<Vec<RecentGuild>> {
    sqlx::query_as!(
        RecentGuild,
        r#"
        SELECT guild_id, name, avatar_url
        FROM guilds
        ORDER BY id DESC
        LIMIT $1
        "#,
        RECENT_LIMIT
    )
    .fetch_all(executor)
    .await
}

/// 退出済み (guilds に行が無い) でデータが残っているギルド。残っている予定の新しい順
pub async fn left_guilds<'e>(executor: impl PgExecutor<'e>) -> sqlx::Result<Vec<LeftGuild>> {
    sqlx::query_as!(
        LeftGuild,
        r#"
        WITH counts AS (
            SELECT guild_id, count(*) AS n, max(created_at) AS last_created
            FROM events GROUP BY guild_id
        ),
        known AS (
            SELECT guild_id FROM guild_config
            UNION SELECT guild_id FROM event_settings
            UNION SELECT guild_id FROM counts
        )
        SELECT k.guild_id AS "guild_id!",
               COALESCE(c.n, 0) AS "event_count!",
               c.last_created AS "last_event_created_at"
        FROM known k
        LEFT JOIN guilds g ON g.guild_id = k.guild_id
        LEFT JOIN counts c ON c.guild_id = k.guild_id
        WHERE g.guild_id IS NULL
        ORDER BY c.last_created DESC NULLS LAST, k.guild_id
        LIMIT $1
        "#,
        RECENT_LIMIT
    )
    .fetch_all(executor)
    .await
}

/// `[day_start, day_end)` (JST) に発火する通知の数。
///
/// Bot の判定 (bot/src/tasks/notify.rs の `fire_at`) と同じく発火時刻は `start_at - num unit`。
/// 対象の予定は `start_at` が今日以降 [`NOTIFICATION_LOOKAHEAD_DAYS`] 日先までのものに限る
pub async fn notifications_between<'e>(
    executor: impl PgExecutor<'e>,
    day_start: NaiveDateTime,
    day_end: NaiveDateTime,
) -> sqlx::Result<i64> {
    let horizon = day_end + chrono::Duration::days(NOTIFICATION_LOOKAHEAD_DAYS);
    let rows = sqlx::query!(
        r#"
        SELECT start_at, notifications
        FROM events
        WHERE start_at >= $1 AND start_at < $2 AND cardinality(notifications) > 0
        "#,
        day_start,
        horizon
    )
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| count_fired_between(row.start_at, &row.notifications, day_start, day_end) as i64)
        .sum())
}

/// 1 件の予定の通知のうち `[day_start, day_end)` に発火するものの数
fn count_fired_between(
    start_at: NaiveDateTime,
    raw_notifications: &[String],
    day_start: NaiveDateTime,
    day_end: NaiveDateTime,
) -> usize {
    Notification::decode_all(raw_notifications)
        .into_iter()
        .filter(|n| {
            // num は u32 で上限を決めていないので、分に直した時点でも減算でも溢れうる。
            // 計算できない通知は Bot 側 (fire_at) でも送られないので数えない
            chrono::Duration::try_minutes(n.total_minutes())
                .and_then(|offset| start_at.checked_sub_signed(offset))
                .is_some_and(|fire| fire >= day_start && fire < day_end)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use super::count_fired_between;

    fn dt(s: &str) -> NaiveDateTime {
        s.parse().unwrap()
    }

    #[test]
    fn counts_notifications_firing_within_the_day() {
        let day_start = dt("2026-08-23T00:00:00");
        let day_end = dt("2026-08-24T00:00:00");
        let notifications = vec![
            // 開始 (8/24 10:00) の 30 分前 = 8/24 09:30 → 今日ではない
            r#"{"key":0,"num":30,"type":"分前"}"#.to_owned(),
            // 1 日前 = 8/23 10:00 → 今日
            r#"{"key":1,"num":1,"type":"日前"}"#.to_owned(),
            // 1 週間前 = 8/17 10:00 → 今日ではない
            r#"{"key":2,"num":1,"type":"週間前"}"#.to_owned(),
            // 24 時間前 = 8/23 10:00 → 今日 (単位が違っても同じ時刻に発火する)
            r#"{"key":3,"num":24,"type":"時間前"}"#.to_owned(),
        ];
        assert_eq!(
            count_fired_between(
                dt("2026-08-24T10:00:00"),
                &notifications,
                day_start,
                day_end
            ),
            2
        );
    }

    #[test]
    fn ignores_unparseable_and_overflowing_notifications() {
        let day_start = dt("2026-08-23T00:00:00");
        let day_end = dt("2026-08-24T00:00:00");
        let notifications = vec![
            "garbage".to_owned(),
            // 桁が大きすぎて日時の演算がオーバーフローする (Bot も送れない)
            r#"{"key":0,"num":4294967295,"type":"週間前"}"#.to_owned(),
        ];
        assert_eq!(
            count_fired_between(
                dt("2026-08-23T10:00:00"),
                &notifications,
                day_start,
                day_end
            ),
            0
        );
    }

    #[test]
    fn counts_the_start_itself_as_a_notification() {
        let day_start = dt("2026-08-23T00:00:00");
        let day_end = dt("2026-08-24T00:00:00");
        let notifications = vec![r#"{"key":0,"num":0,"type":"分前"}"#.to_owned()];
        assert_eq!(
            count_fired_between(
                dt("2026-08-23T10:00:00"),
                &notifications,
                day_start,
                day_end
            ),
            1
        );
    }
}
