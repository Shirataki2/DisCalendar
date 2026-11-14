//! 管理コンソールの分析情報 (#79) の DB テスト。
//! `#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する (Postgres が必要)。
//! Better Auth のテーブル (`user`) はマイグレーションに含まれないのでここで作る。
//!
//! 集計の基準時刻は固定値を渡して、実行した日時に結果が左右されないようにしている
//! (基準は JST の 2026-08-25 12:00 = UTC の 2026-08-25 03:00)。

use chrono::{NaiveDate, NaiveDateTime};
use discalendar_api::models::{admin_analytics, user_retention};
use sqlx::PgPool;

/// 集計の基準時刻 (JST)。予定の `created_at` はタイムゾーンなしの JST なのでこちらで比べる
const NOW_JST: &str = "2026-08-25T12:00:00";

const GUILD_A: &str = "111111111111111111";
const GUILD_B: &str = "222222222222222222";
/// 予定は残っているが `guilds` に行が無い (Bot が退出した) ギルド
const GUILD_LEFT: &str = "333333333333333333";
/// 予定が 1 件も無いギルド
const GUILD_IDLE: &str = "444444444444444444";

fn now_jst() -> NaiveDateTime {
    NOW_JST.parse().unwrap()
}

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

/// 直近 30 日の起点 (`guild_counts` / `top_guilds` に渡す)
fn recent_since() -> NaiveDateTime {
    now_jst() - chrono::Duration::days(admin_analytics::RECENT_DAYS)
}

/// Better Auth の `user` を本番と同じ列で用意する (`createdAt` は UTC)
async fn create_user_table(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        CREATE TABLE "user" (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL,
            "emailVerified" BOOLEAN NOT NULL DEFAULT false, image TEXT,
            "createdAt" TIMESTAMPTZ NOT NULL DEFAULT now(), "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT now()
        );
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

/// 利用者を入れる (新規ユーザーの集計用)
async fn seed_auth(pool: &PgPool) {
    create_user_table(pool).await;
    sqlx::raw_sql(
        r#"
        INSERT INTO "user" (id, name, email, "createdAt") VALUES
            ('u1', 'Alice', 'alice@example.com', '2026-08-25T00:00:00Z'),
            ('u2', 'Bob',   'bob@example.com',   '2026-08-24T10:00:00Z'),
            ('u3', 'Carol', 'carol@example.com', '2026-07-01T00:00:00Z'),
            ('u4', 'Dave',  'dave@example.com',  '2026-08-20T00:00:00Z');
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

/// ギルド・予定を入れる。予定の `created_at` は基準時刻 (08-25 12:00 JST) から見て
/// 直近 1 日 / 7 日 / 30 日とその前の期間に 1 件ずつ入るように置く。
///
/// - 通知先チャンネル: GUILD_A は正しい形式の ID、GUILD_B は旧データの `"0"` (不正)、GUILD_IDLE は未設定
/// - restricted: GUILD_A だけ true
async fn seed_guilds(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO guilds (guild_id, name) VALUES
            ('111111111111111111', 'メインサーバー'),
            ('222222222222222222', 'サブサーバー'),
            ('444444444444444444', '予定なしサーバー');
        INSERT INTO event_settings (guild_id, channel_id) VALUES
            ('111111111111111111', '555555555555555555'),
            ('222222222222222222', '0');
        INSERT INTO guild_config (guild_id, restricted) VALUES
            ('111111111111111111', true),
            ('222222222222222222', false);
        INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at, created_at) VALUES
            -- 直近 24 時間 (08-24 12:00 以降)
            ('111111111111111111', 'A', '[{"num":1,"unit":"days"},{"num":30,"unit":"minutes"}]'::jsonb,
             true,  '2026-08-26T00:00:00', '2026-08-26T00:00:00', '2026-08-25T09:00:00'),
            -- その前の 24 時間 (08-23 12:00 〜 08-24 12:00)
            ('111111111111111111', 'B', '[]'::jsonb,
             false, '2026-08-26T10:00:00', '2026-08-26T11:00:00', '2026-08-24T06:00:00'),
            -- 直近 7 日
            ('111111111111111111', 'C', '[{"num":1,"unit":"hours"}]'::jsonb,
             false, '2026-08-27T10:00:00', '2026-08-27T11:00:00', '2026-08-20T09:00:00'),
            -- その前の 7 日
            ('222222222222222222', 'D', '[]'::jsonb,
             false, '2026-08-28T10:00:00', '2026-08-28T11:00:00', '2026-08-12T09:00:00'),
            -- 直近 30 日
            ('222222222222222222', 'E', '[]'::jsonb,
             false, '2026-08-29T10:00:00', '2026-08-29T11:00:00', '2026-08-01T09:00:00'),
            ('333333333333333333', 'G', '[]'::jsonb,
             false, '2026-08-30T10:00:00', '2026-08-30T11:00:00', '2026-08-05T09:00:00'),
            -- その前の 30 日 (日別 30 日の窓からは外れる)
            ('333333333333333333', 'F', '[]'::jsonb,
             false, '2026-08-31T10:00:00', '2026-08-31T11:00:00', '2026-07-01T09:00:00');
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

/// 旧データや手動修正で入りうる「解釈できない通知設定」だけを持つ予定。
/// `Notification::decode_all` が全部捨てるので、Bot は開始時刻の通知しか送らない
async fn seed_broken_notifications(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at, created_at) VALUES
            -- JSON として壊れている
            ('111111111111111111', 'X', '["garbage"]'::jsonb,
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00'),
            -- 単位が未知
            ('111111111111111111', 'Y', '[{"num":2,"unit":"years"}]'::jsonb,
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00'),
            -- num が負で u32 にできない
            ('111111111111111111', 'Z', '[{"num":-1,"unit":"minutes"}]'::jsonb,
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00'),
            -- u32 としては有効だが、分に直すと日時の計算ができない (Bot も送れない)。
            -- api は num の値域を検証していない (#46) ので API 直叩きや旧データで入りうる
            ('111111111111111111', 'V', '[{"num":4294967295,"unit":"weeks"}]'::jsonb,
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00');
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

/// 壊れた設定と有効な設定が混ざっている予定
async fn seed_partially_broken_notification(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at, created_at) VALUES
            ('111111111111111111', 'W', '["garbage", {"num":5,"unit":"minutes"}]'::jsonb,
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00');
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

/// 発火時刻が重なる設定を持つ予定 (Bot は 1 回にまとめて送る)
async fn seed_duplicate_notifications(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at, created_at) VALUES
            -- 「60 分前」と「1 時間前」は同じ発火時刻。保存済みの「0 分前」は開始時刻の通知と同じ
            ('111111111111111111', 'D2',
             '[{"num":60,"unit":"minutes"}, {"num":1,"unit":"hours"}, {"num":0,"unit":"minutes"}]'::jsonb,
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00'),
            -- 保存済みの「0 分前」だけ = 追加の通知は無い
            ('111111111111111111', 'D3', '[{"num":0,"unit":"minutes"}]'::jsonb,
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00');
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

/// 利用の記録 (#81) を入れる。基準日 (JST の 2026-08-25) から見て各期間に散らす:
///
/// | 利用者 | 日付 | 入る期間 |
/// |---|---|---|
/// | u1 | 08-25, 08-24 | 今日・昨日・直近 7 日・直近 30 日 |
/// | u2 | 08-25 | 今日・直近 7 日・直近 30 日 |
/// | u3 | 08-15 | その前の 7 日 (08-12〜08-18)・直近 30 日 |
/// | u4 | 07-01 | その前の 30 日 (06-27〜07-26) だけ |
/// | u5 | 08-26 | 基準日より未来 (どの期間にも入れない) |
async fn seed_activity(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO user_daily_activity (user_id, day) VALUES
            ('u1', '2026-08-25'), ('u1', '2026-08-24'),
            ('u2', '2026-08-25'),
            ('u3', '2026-08-15'),
            ('u4', '2026-07-01'),
            ('u5', '2026-08-26');
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn active_users_count_recorded_days_in_each_window(pool: PgPool) {
    seed_activity(&pool).await;

    let active = admin_analytics::active_users(&pool, date("2026-08-25"))
        .await
        .unwrap();

    // 記録が始まった日は一番古い行。基準日より未来の行 (u5) はどの集計にも入らない
    assert_eq!(active.since, Some(date("2026-07-01")));
    // 今日は u1 / u2、昨日は u1 だけ
    assert_eq!(active.daily.current, 2);
    assert_eq!(active.daily.previous, 1);
    // 直近 7 日 (08-19〜08-25) は u1 / u2、その前の 7 日 (08-12〜08-18) は u3
    assert_eq!(active.weekly.current, 2);
    assert_eq!(active.weekly.previous, 1);
    // 直近 30 日 (07-27〜08-25) は u1 / u2 / u3、その前の 30 日 (06-27〜07-26) は u4
    assert_eq!(active.monthly.current, 3);
    assert_eq!(active.monthly.previous, 1);
    assert_eq!(active.monthly.delta, 2);
    assert_eq!(active.monthly.change_percent, Some(200.0));
    // DAU/MAU 比は昨日までの 30 日 (07-26〜08-24) で取る。今日の u1 / u2 は入らない。
    // その期間は u1 (08-24) と u3 (08-15) の延べ 2 人・日、利用者 2 人 → 2 / 30 / 2
    let stickiness = active.stickiness.unwrap();
    assert!((stickiness - 1.0 / 30.0).abs() < 1e-9, "{stickiness}");
}

#[sqlx::test(migrations = "./migrations")]
async fn active_users_are_empty_before_any_record(pool: PgPool) {
    let active = admin_analytics::active_users(&pool, date("2026-08-25"))
        .await
        .unwrap();

    // 記録が始まる前は「記録なし」と分かる形で返す (0 と null。増減率も比も出さない)
    assert_eq!(active.since, None);
    assert_eq!(active.daily.current, 0);
    assert_eq!(active.monthly.current, 0);
    assert_eq!(active.monthly.change_percent, None);
    assert_eq!(active.stickiness, None);
}

#[sqlx::test(migrations = "./migrations")]
async fn event_creation_counts_each_window(pool: PgPool) {
    seed_guilds(&pool).await;

    let (creation, all_day) = admin_analytics::event_creation(&pool, now_jst())
        .await
        .unwrap();

    assert_eq!(creation.total, 7);
    // 直近 24 時間は A、その前の 24 時間は B
    assert_eq!(creation.last_day.current, 1);
    assert_eq!(creation.last_day.previous, 1);
    // 直近 7 日は A / B / C、その前の 7 日は D
    assert_eq!(creation.last_week.current, 3);
    assert_eq!(creation.last_week.previous, 1);
    // 直近 30 日は A / B / C / D / E / G、その前の 30 日は F
    assert_eq!(creation.last_month.current, 6);
    assert_eq!(creation.last_month.previous, 1);

    assert_eq!(all_day, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn event_creation_is_zero_without_events(pool: PgPool) {
    let (creation, all_day) = admin_analytics::event_creation(&pool, now_jst())
        .await
        .unwrap();

    assert_eq!(creation.total, 0);
    assert_eq!(creation.last_month.current, 0);
    // 分母が 0 のときに増減率を出さない (0 除算にしない)
    assert_eq!(creation.last_month.change_percent, None);
    assert_eq!(all_day, 0);
    assert_eq!(
        admin_analytics::notification_stats(&pool).await.unwrap(),
        (0, 0)
    );
}

/// 配列の長さではなく、Bot が実際に発火させる通知 (`Notification::fire_minutes`) から
/// 必ず送られる開始時刻の通知を除いた数を数えていること
#[sqlx::test(migrations = "./migrations")]
async fn notification_stats_count_only_settings_that_actually_notify(pool: PgPool) {
    seed_guilds(&pool).await;

    // 通知を設定しているのは A (1 日前 + 30 分前 = 2 件) と C (1 時間前 = 1 件)
    assert_eq!(
        admin_analytics::notification_stats(&pool).await.unwrap(),
        (2, 3)
    );

    seed_broken_notifications(&pool).await;
    // 壊れた JSON / 未知の単位 / 負の num / 発火時刻を計算できない num しか持たない予定は数に入らない
    assert_eq!(
        admin_analytics::notification_stats(&pool).await.unwrap(),
        (2, 3)
    );

    seed_partially_broken_notification(&pool).await;
    // 壊れた設定と有効な設定が混ざっている予定は、有効な方だけ数える
    assert_eq!(
        admin_analytics::notification_stats(&pool).await.unwrap(),
        (3, 4)
    );

    seed_duplicate_notifications(&pool).await;
    // 「60 分前」と「1 時間前」は Bot が 1 回にまとめるので 1 件。
    // 保存済みの「0 分前」は必ず送られる開始時刻の通知と同じなので数に入らない
    assert_eq!(
        admin_analytics::notification_stats(&pool).await.unwrap(),
        (4, 5)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn daily_series_covers_every_day_including_empty_ones(pool: PgPool) {
    seed_auth(&pool).await;
    seed_guilds(&pool).await;
    seed_activity(&pool).await;

    let daily = admin_analytics::daily(&pool, now_jst()).await.unwrap();

    // 予定が無い日も 0 の点として並ぶ (グラフに穴を空けない)
    assert_eq!(daily.len() as i32, admin_analytics::DAILY_DAYS);
    assert_eq!(daily[0].date, date("2026-07-27"));
    assert_eq!(daily[daily.len() - 1].date, date("2026-08-25"));

    let on = |d: &str| daily.iter().find(|p| p.date == date(d)).unwrap();
    assert_eq!(on("2026-08-25").events, 1); // A
    assert_eq!(on("2026-08-24").events, 1); // B
    assert_eq!(on("2026-08-23").events, 0);
    // 窓から外れた F (07-01) は含まれず、G (08-05) は含まれる
    assert_eq!(daily.iter().map(|p| p.events).sum::<i64>(), 6);

    // 新規ユーザーは JST の日付で分ける (u2 の 08-24 10:00Z は JST では 19:00)
    assert_eq!(on("2026-08-25").new_users, 1); // u1
    assert_eq!(on("2026-08-24").new_users, 1); // u2
    assert_eq!(on("2026-08-20").new_users, 1); // u4

    // アクティブユーザー (#81)。記録が無い日は 0 で、
    // 窓から外れた u4 (07-01) と今日 (右端) より未来の u5 (08-26) は入らない
    assert_eq!(on("2026-08-25").active_users, 2); // u1 / u2
    assert_eq!(on("2026-08-24").active_users, 1); // u1
    assert_eq!(on("2026-08-15").active_users, 1); // u3
    assert_eq!(on("2026-08-23").active_users, 0);
    assert_eq!(daily.iter().map(|p| p.active_users).sum::<i64>(), 4);
}

#[sqlx::test(migrations = "./migrations")]
async fn monthly_series_buckets_by_jst_month(pool: PgPool) {
    seed_auth(&pool).await;
    seed_guilds(&pool).await;
    seed_activity(&pool).await;

    let monthly = admin_analytics::monthly(&pool, now_jst()).await.unwrap();

    assert_eq!(monthly.len() as i32, admin_analytics::MONTHLY_MONTHS);
    assert_eq!(monthly[0].month, date("2025-09-01"));
    assert_eq!(monthly[monthly.len() - 1].month, date("2026-08-01"));

    let on = |m: &str| monthly.iter().find(|p| p.month == date(m)).unwrap();
    let august = on("2026-08-01");
    assert_eq!(august.events, 6); // F 以外
    assert_eq!(august.new_users, 3); // u1 / u2 / u4
    // u1 は 2 日使っているが 1 人。今日より未来の u5 (08-26) は入らない
    assert_eq!(august.active_users, 3); // u1 / u2 / u3

    let july = on("2026-07-01");
    assert_eq!(july.events, 1); // F
    assert_eq!(july.new_users, 1); // u3
    assert_eq!(july.active_users, 1); // u4
    // 記録の無い月は 0
    assert_eq!(on("2026-06-01").active_users, 0);
}

/// 予定の作成元は `event_history` の `create` だけを、期間で絞って数える
#[sqlx::test(migrations = "./migrations")]
async fn creation_sources_count_create_history_by_source(pool: PgPool) {
    seed_guilds(&pool).await;
    // 予定に履歴を付ける。created_at は TIMESTAMPTZ (JST の 08-25 12:00 = 03:00Z が基準)
    sqlx::raw_sql(
        r#"
        INSERT INTO event_history (event_id, guild_id, source, action, after, created_at)
        SELECT e.id, e.guild_id, h.src, h.act, '{}'::jsonb, h.at::timestamptz
        FROM events e
        JOIN (VALUES
            ('A', 'web', 'create', '2026-08-25T00:00:00Z'),
            ('A', 'web', 'update', '2026-08-25T01:00:00Z'),
            ('B', 'bot', 'create', '2026-08-24T00:00:00Z'),
            ('C', 'bot', 'create', '2026-08-20T00:00:00Z'),
            ('D', 'mcp', 'create', '2026-08-12T00:00:00Z'),
            -- 30 日より前
            ('F', 'admin', 'create', '2026-07-01T00:00:00Z'),
            -- 基準時刻より後
            ('G', 'web', 'create', '2026-08-25T04:00:00Z')
        ) AS h(name, src, act, at) ON h.name = e.name;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let sources = admin_analytics::creation_sources(&pool, recent_since(), now_jst())
        .await
        .unwrap();
    let pairs: Vec<(&str, i64)> = sources
        .iter()
        .map(|s| (s.source.as_str(), s.count))
        .collect();
    // 多い順。更新・期間外・未来の行は数えない
    assert_eq!(pairs, vec![("bot", 2), ("mcp", 1), ("web", 1)]);
}

/// 新規ユーザーの定着: 登録日当日の利用は数えず、期間が過ぎ切った人だけを分母にする
#[sqlx::test(migrations = "./migrations")]
async fn user_retention_counts_returns_after_registration(pool: PgPool) {
    create_user_table(&pool).await;
    // 基準日は JST の 2026-08-25。登録は JST の 0 時台 (UTC の前日 15 時台) に置く
    sqlx::raw_sql(
        r#"
        INSERT INTO "user" (id, name, email, "createdAt") VALUES
            -- 記録開始 (07-01) より前の登録は数えない
            ('old', 'Old', 'old@example.com', '2026-06-20T00:00:00Z'),
            -- 7 月登録: 30 日が過ぎ切っている
            ('a', 'A', 'a@example.com', '2026-07-01T15:30:00Z'), -- JST 07-02
            ('b', 'B', 'b@example.com', '2026-07-09T15:30:00Z'), -- JST 07-10
            -- 8 月登録: 08-17 は 7 日が過ぎ切っている (08-24 まで)、08-20 はまだ
            ('c', 'C', 'c@example.com', '2026-08-16T15:30:00Z'), -- JST 08-17
            ('d', 'D', 'd@example.com', '2026-08-19T15:30:00Z'); -- JST 08-20
        INSERT INTO user_daily_activity (user_id, day) VALUES
            ('old', '2026-07-01'),
            -- a: 登録日だけでなく翌日にも使い、20 日目にも戻ってきた
            ('a', '2026-07-02'), ('a', '2026-07-03'), ('a', '2026-07-21'),
            -- b: 登録日にしか使っていない
            ('b', '2026-07-10'),
            -- c: 7 日目 (08-24) に使った。直近 30 日にも入る
            ('c', '2026-08-17'), ('c', '2026-08-24'),
            -- d: 3 日目に使ったが、まだ 7 日が過ぎ切っていないので分母にも分子にも入らない
            ('d', '2026-08-22');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let retention =
        user_retention::user_retention(&mut conn, now_jst(), admin_analytics::RECENT_DAYS)
            .await
            .unwrap();

    assert_eq!(retention.since, Some(date("2026-07-01")));
    let months: Vec<_> = retention.cohorts.iter().map(|c| c.month).collect();
    assert_eq!(months, vec![date("2026-08-01"), date("2026-07-01")]);

    let august = &retention.cohorts[0];
    assert_eq!(august.users, 2);
    assert_eq!(august.week_eligible, 1); // c
    assert_eq!(august.week_returned, 1); // c (08-24)
    assert_eq!(august.month_eligible, 0);
    assert_eq!(august.active, 2); // c / d

    let july = &retention.cohorts[1];
    assert_eq!(july.users, 2); // a / b (old は記録開始前の登録なので入らない)
    assert_eq!(july.week_eligible, 2);
    assert_eq!(july.week_returned, 1); // a
    assert_eq!(july.month_eligible, 2);
    assert_eq!(july.month_returned, 1); // a (07-21)
    // 直近 30 日 (07-27〜08-25) には使っていない
    assert_eq!(july.active, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn user_retention_is_empty_before_any_record(pool: PgPool) {
    seed_auth(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    let retention =
        user_retention::user_retention(&mut conn, now_jst(), admin_analytics::RECENT_DAYS)
            .await
            .unwrap();
    assert_eq!(retention.since, None);
    assert!(retention.cohorts.is_empty());
}

/// 集計の基準時刻より後の `created_at` を持つ予定を、どの集計にも入れないこと。
/// (トランザクションを開くまでの間に作られた行、時計のずれ、手動投入で起こりうる)
#[sqlx::test(migrations = "./migrations")]
async fn future_dated_events_are_excluded_from_every_window(pool: PgPool) {
    seed_auth(&pool).await;
    seed_guilds(&pool).await;
    sqlx::raw_sql(
        r#"
        INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at, created_at) VALUES
            -- 基準の 08-25 12:00 より後 (同じ「今日」ではあるが集計時点より未来)
            ('222222222222222222', 'F1', '[]'::jsonb,
             false, '2026-09-10T10:00:00', '2026-09-10T11:00:00', '2026-08-25T23:00:00'),
            -- ずっと先の日付が入っている行
            ('333333333333333333', 'F2', '[]'::jsonb,
             false, '2027-01-01T10:00:00', '2027-01-01T11:00:00', '2027-01-01T00:00:00');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let (creation, _) = admin_analytics::event_creation(&pool, now_jst())
        .await
        .unwrap();
    // 総数には入る (今 DB にある予定の数なので) が、期間の集計には入らない
    assert_eq!(creation.total, 9);
    assert_eq!(creation.last_day.current, 1);
    assert_eq!(creation.last_month.current, 6);

    // 推移の右端 (今日 / 当月) も同じ基準で切る。ここがずれると画面の合計と件数が食い違う
    let daily = admin_analytics::daily(&pool, now_jst()).await.unwrap();
    assert_eq!(daily[daily.len() - 1].date, date("2026-08-25"));
    assert_eq!(daily[daily.len() - 1].events, 1);

    let monthly = admin_analytics::monthly(&pool, now_jst()).await.unwrap();
    let august = monthly
        .iter()
        .find(|m| m.month == date("2026-08-01"))
        .unwrap();
    assert_eq!(august.events, 6);

    // 未来日時の予定 1 件でギルドをアクティブ扱いにしない
    let (active_guilds, active_left_guilds, _) =
        admin_analytics::guild_counts(&pool, recent_since(), now_jst())
            .await
            .unwrap();
    assert_eq!(active_guilds, 2);
    assert_eq!(active_left_guilds, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn guild_activity_ranks_guilds_by_recent_events(pool: PgPool) {
    seed_guilds(&pool).await;

    let (active_guilds, active_left_guilds, joined_guilds) =
        admin_analytics::guild_counts(&pool, recent_since(), now_jst())
            .await
            .unwrap();

    // 直近 30 日に予定が作られたのは A (3 件) / B (2 件) / 退出済み (1 件)。予定なしサーバーは入らない。
    // 割合 (参加中のうちどれだけ使われているか) を出せるよう、退出済みは分子に混ぜない
    assert_eq!(active_guilds, 2);
    assert_eq!(active_left_guilds, 1);
    assert_eq!(joined_guilds, 3);
    // 分子が分母を超えない (退出済みを混ぜると 3 / 3 = 100% と読めてしまう)
    assert!(active_guilds <= joined_guilds);

    let top = admin_analytics::top_guilds(&pool, recent_since(), now_jst())
        .await
        .unwrap();
    assert_eq!(top.len(), 3);
    assert_eq!(top[0].guild_id, GUILD_A);
    assert_eq!(top[0].event_count, 3);
    assert_eq!(top[0].name.as_deref(), Some("メインサーバー"));
    assert_eq!(top[1].guild_id, GUILD_B);
    assert_eq!(top[1].event_count, 2);
    // 退出済みのギルドは guilds に行が無いので名前が出せない
    assert_eq!(top[2].guild_id, GUILD_LEFT);
    assert_eq!(top[2].event_count, 1);
    assert_eq!(top[2].name, None);
    assert!(!top.iter().any(|g| g.guild_id == GUILD_IDLE));
}

#[sqlx::test(migrations = "./migrations")]
async fn guild_settings_only_count_channel_ids_in_a_valid_format(pool: PgPool) {
    seed_guilds(&pool).await;

    let (with_channel, restricted) = admin_analytics::guild_settings(&pool).await.unwrap();

    // GUILD_B の channel_id は旧データの "0" で Snowflake として不正。GUILD_IDLE は未設定
    assert_eq!(with_channel, 1);
    assert_eq!(restricted, 1);
}
