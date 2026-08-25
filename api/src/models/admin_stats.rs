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
/// 通知は「開始の num unit 前」に飛ぶので、今日発火する通知を持つ予定は今日以降のいくらでも先に
/// あり得る。全件走査を避けるため、web のフォームが許す最大の事前通知
/// (`web/src/lib/event-form.ts` の `NOTIFICATION_NUM_MAX` = 100 × 週 = 700 日前) までを対象にする。
/// api 側は `num` の値域を検証していないので、これを超える設定 (API 直叩きや旧データ) だけは数に入らない
pub const NOTIFICATION_LOOKAHEAD_DAYS: i64 = 700;

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
    /// まだ終わっていない予定。終日予定の `end_at` は「終了日 (含む) の 0:00」で保存されるので
    /// (web/src/lib/calendar-events.ts の `toApiRange`)、その日のうちは終わっていない扱いにする
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
            (SELECT count(*) FROM events WHERE
                CASE WHEN is_all_day
                     -- 終日予定は終了日の 0:00 が入っているので、その日いっぱいは残す
                     THEN date_trunc('day', end_at) + interval '1 day' > $1
                     ELSE end_at >= $1
                END
            ) AS "upcoming_events!",
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

/// `[day_start, day_end)` (JST) に Bot が送る通知の数。
///
/// 判定は Bot (bot/src/tasks/notify.rs の `notify_for_event`) に合わせる:
///
/// - 発火時刻は `開始 - num unit`。終日予定の開始は 0:00 に丸める (`effective_range`)
/// - 保存済みの設定に加えて**必ず開始時刻 (0 分前) の通知**を送り、同じ分数のものは 1 回にまとめる
/// - 通知先チャンネル (`event_settings` の先頭の行) が無いギルドや、`channel_id` が
///   Snowflake として不正なギルド (旧データの `"0"` など) には送らないので数に入れない
/// - Bot が退出したギルド (`guilds` に行が無い) は、通知先の設定と予定が残っていても
///   送信が Missing Access になって届かないので数に入れない
///
/// 対象の予定は `start_at` が今日以降 [`NOTIFICATION_LOOKAHEAD_DAYS`] 日先までのものに限る
/// (終日予定の丸めで発火が最大 1 日手前にずれる分だけ広く取る)。
///
/// 参加中かどうかの判定は `guilds` テーブル (Bot 自身の記録) だけで行い、Discord API には問い合わせない。
/// 概要は運用時に最初に開く画面なので、全ギルドを辿る重い呼び出しと Discord 障害への依存を持ち込まないため。
/// そのため「再参加したが `guilds` への反映を取りこぼした」ギルドの通知は数に入らないが、
/// そのずれ自体は `GET /admin/guilds/sync-check` (差分検出) で明示的に確認できる
pub async fn notifications_between<'e>(
    executor: impl PgExecutor<'e>,
    day_start: NaiveDateTime,
    day_end: NaiveDateTime,
) -> sqlx::Result<i64> {
    let horizon = day_end + chrono::Duration::days(NOTIFICATION_LOOKAHEAD_DAYS + 1);
    let rows = sqlx::query!(
        r#"
        WITH channels AS (
            -- 旧スキーマには guild_id の一意制約が無いので、Bot (event_settings::get) と同じく先頭の 1 行を使う
            SELECT DISTINCT ON (guild_id) guild_id, channel_id
            FROM event_settings ORDER BY guild_id, id
        )
        SELECT e.start_at, e.is_all_day, e.notifications, c.channel_id
        FROM events e
        JOIN channels c ON c.guild_id = e.guild_id
        -- Bot が退出したギルド (guilds に行が無い) には送れない
        JOIN guilds g ON g.guild_id = e.guild_id
        WHERE e.start_at >= $1 AND e.start_at < $2
        "#,
        day_start,
        horizon
    )
    .fetch_all(executor)
    .await?;

    Ok(rows
        .into_iter()
        .filter(|row| is_sendable_channel(&row.channel_id))
        .map(|row| {
            count_fired_between(
                row.start_at,
                row.is_all_day,
                &row.notifications,
                day_start,
                day_end,
            ) as i64
        })
        .sum())
}

/// Bot が実際に送信先にできるチャンネル ID か。
/// `event_settings.channel_id` は制約のない TEXT なので、旧データや手動修正で `"0"` や
/// 数値でない値が入りうる。Bot (bot/src/tasks/notify.rs) と同じく `NonZeroU64` で判定する。
/// 分析情報 (`admin_analytics`) の「通知先設定済みギルド」も同じ判定を使う
pub fn is_sendable_channel(channel_id: &str) -> bool {
    channel_id.parse::<std::num::NonZeroU64>().is_ok()
}

/// 1 件の予定について、Bot が `[day_start, day_end)` に送る通知の数
fn count_fired_between(
    start_at: NaiveDateTime,
    is_all_day: bool,
    raw_notifications: &[String],
    day_start: NaiveDateTime,
    day_end: NaiveDateTime,
) -> usize {
    // 終日予定は開始日の 0:00 を基準にする (web / api / Bot 共通の規約)
    let start = if is_all_day {
        start_at
            .date()
            .and_hms_opt(0, 0, 0)
            .expect("valid midnight")
    } else {
        start_at
    };
    let mut minutes: Vec<i64> = Notification::decode_all(raw_notifications)
        .into_iter()
        .map(Notification::total_minutes)
        .collect();
    // Bot は保存済みの設定に関係なく開始時刻の通知を送る
    minutes.push(0);
    // 「60 分前」と「1 時間前」のように分数が同じものは Bot も 1 回にまとめる
    minutes.sort_unstable();
    minutes.dedup();

    minutes
        .into_iter()
        .filter(|&m| {
            // num は u32 で上限を決めていないので、分に直した時点でも減算でも溢れうる。
            // 計算できない通知は Bot 側 (fire_at) でも送られないので数えない
            chrono::Duration::try_minutes(m)
                .and_then(|offset| start.checked_sub_signed(offset))
                .is_some_and(|fire| fire >= day_start && fire < day_end)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;

    use super::{count_fired_between, is_sendable_channel};

    fn dt(s: &str) -> NaiveDateTime {
        s.parse().unwrap()
    }

    /// 8/23 の 1 日ぶんを判定窓にする
    fn count(start: &str, is_all_day: bool, notifications: &[String]) -> usize {
        count_fired_between(
            dt(start),
            is_all_day,
            notifications,
            dt("2026-08-23T00:00:00"),
            dt("2026-08-24T00:00:00"),
        )
    }

    #[test]
    fn counts_notifications_firing_within_the_day() {
        let notifications = vec![
            // 開始 (8/24 10:00) の 30 分前 = 8/24 09:30 → 今日ではない
            r#"{"key":0,"num":30,"type":"分前"}"#.to_owned(),
            // 1 日前 = 8/23 10:00 → 今日
            r#"{"key":1,"num":1,"type":"日前"}"#.to_owned(),
            // 1 週間前 = 8/17 10:00 → 今日ではない
            r#"{"key":2,"num":1,"type":"週間前"}"#.to_owned(),
            // 24 時間前 = 8/23 10:00 → 今日だが「1 日前」と同じ時刻なので Bot は 1 回しか送らない
            r#"{"key":3,"num":24,"type":"時間前"}"#.to_owned(),
        ];
        assert_eq!(count("2026-08-24T10:00:00", false, &notifications), 1);
    }

    #[test]
    fn ignores_unparseable_and_overflowing_notifications() {
        let notifications = vec![
            "garbage".to_owned(),
            // 桁が大きすぎて日時の演算がオーバーフローする (Bot も送れない)
            r#"{"key":0,"num":4294967295,"type":"週間前"}"#.to_owned(),
        ];
        // 残るのは Bot が必ず送る開始時刻の通知だけ
        assert_eq!(count("2026-08-23T10:00:00", false, &notifications), 1);
        assert_eq!(count("2026-08-24T10:00:00", false, &notifications), 0);
    }

    #[test]
    fn always_counts_the_start_notification_the_bot_adds() {
        // 設定が無くても Bot は開始時刻に通知する
        assert_eq!(count("2026-08-23T10:00:00", false, &[]), 1);
        // 保存済みの 0 分前と重複しても 1 回 (Bot も dedup する)
        let zero = vec![r#"{"key":0,"num":0,"type":"分前"}"#.to_owned()];
        assert_eq!(count("2026-08-23T10:00:00", false, &zero), 1);
    }

    #[test]
    fn rejects_channel_ids_the_bot_cannot_send_to() {
        assert!(is_sendable_channel("782502586817314820"));
        // 旧データや手動修正で入りうる値 (Bot も NonZeroU64 で弾いて送らない)
        assert!(!is_sendable_channel("0"));
        assert!(!is_sendable_channel(""));
        assert!(!is_sendable_channel("general"));
        assert!(!is_sendable_channel("-1"));
    }

    #[test]
    fn all_day_events_fire_from_midnight() {
        let notifications = vec![r#"{"key":0,"num":30,"type":"分前"}"#.to_owned()];
        // 終日予定の開始は 0:00 に丸められるので、8/24 の予定の 30 分前は 8/23 23:30 = 今日
        assert_eq!(count("2026-08-24T15:30:00", true, &notifications), 1);
        // 終日でなければ 8/24 09:30 なので今日ではない (開始通知も 8/24)
        assert_eq!(count("2026-08-24T15:30:00", false, &notifications), 0);
    }
}
