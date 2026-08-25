//! 管理コンソールの分析情報 (#79) の DB テスト。
//! `#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する (Postgres が必要)。
//! Better Auth のテーブル (`user` / `session`) はマイグレーションに含まれないのでここで作る。
//!
//! 集計の基準時刻は固定値を渡して、実行した日時に結果が左右されないようにしている
//! (基準は JST の 2026-08-25 12:00 = UTC の 2026-08-25 03:00)。

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use discalendar_api::models::admin_analytics;
use sqlx::PgPool;

/// 集計の基準時刻 (JST)。予定の `created_at` はタイムゾーンなしの JST なのでこちらで比べる
const NOW_JST: &str = "2026-08-25T12:00:00";
/// 同じ時刻を UTC で表したもの。`session` / `user` は TIMESTAMPTZ なのでこちらで比べる
const NOW_UTC: &str = "2026-08-25T03:00:00Z";

const GUILD_A: &str = "111111111111111111";
const GUILD_B: &str = "222222222222222222";
/// 予定は残っているが `guilds` に行が無い (Bot が退出した) ギルド
const GUILD_LEFT: &str = "333333333333333333";
/// 予定が 1 件も無いギルド
const GUILD_IDLE: &str = "444444444444444444";

fn now_jst() -> NaiveDateTime {
    NOW_JST.parse().unwrap()
}

fn now_utc() -> DateTime<Utc> {
    NOW_UTC.parse().unwrap()
}

fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}

/// 直近 30 日の起点 (`guild_counts` / `top_guilds` に渡す)
fn recent_since() -> NaiveDateTime {
    now_jst() - chrono::Duration::days(admin_analytics::RECENT_DAYS)
}

/// Better Auth のテーブルを本番と同じ列で用意し、利用者とセッションを入れる。
///
/// セッションの生存期間 `[createdAt, updatedAt]` が「その利用者が使っていた期間」になるように置く:
///
/// | | createdAt (UTC) | updatedAt (UTC) | 重なる期間 |
/// |---|---|---|---|
/// | s1 (u1) | 08-01 00:00 | 08-25 02:00 | 直近 1 日・その前・7 日・その前・30 日 |
/// | s5 (u1) | 08-24 11:00 | 08-25 01:00 | 直近 1 日 (u1 の重複。DISTINCT の確認用) |
/// | s2 (u2) | 08-24 10:00 | 08-24 10:00 | 直近 1 日・7 日・30 日 |
/// | s4 (u4) | 08-20 00:00 | 08-23 12:00 | 直近 1 日の前・7 日・30 日 |
/// | s3 (u3) | 07-01 00:00 | 07-10 00:00 | 30 日の前だけ |
async fn seed_auth(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        CREATE TABLE "user" (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL,
            "emailVerified" BOOLEAN NOT NULL DEFAULT false, image TEXT,
            "createdAt" TIMESTAMPTZ NOT NULL DEFAULT now(), "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT now()
        );
        CREATE TABLE "session" (
            id TEXT PRIMARY KEY, "expiresAt" TIMESTAMPTZ NOT NULL, token TEXT NOT NULL,
            "createdAt" TIMESTAMPTZ NOT NULL DEFAULT now(), "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT now(),
            "ipAddress" TEXT, "userAgent" TEXT, "userId" TEXT NOT NULL
        );
        INSERT INTO "user" (id, name, email, "createdAt") VALUES
            ('u1', 'Alice', 'alice@example.com', '2026-08-25T00:00:00Z'),
            ('u2', 'Bob',   'bob@example.com',   '2026-08-24T10:00:00Z'),
            ('u3', 'Carol', 'carol@example.com', '2026-07-01T00:00:00Z'),
            ('u4', 'Dave',  'dave@example.com',  '2026-08-20T00:00:00Z');
        INSERT INTO "session" (id, "expiresAt", token, "userId", "createdAt", "updatedAt") VALUES
            ('s1', '2026-09-01T00:00:00Z', 't1', 'u1', '2026-08-01T00:00:00Z', '2026-08-25T02:00:00Z'),
            ('s2', '2026-09-01T00:00:00Z', 't2', 'u2', '2026-08-24T10:00:00Z', '2026-08-24T10:00:00Z'),
            ('s3', '2026-07-17T00:00:00Z', 't3', 'u3', '2026-07-01T00:00:00Z', '2026-07-10T00:00:00Z'),
            ('s4', '2026-08-30T00:00:00Z', 't4', 'u4', '2026-08-20T00:00:00Z', '2026-08-23T12:00:00Z'),
            ('s5', '2026-09-01T00:00:00Z', 't5', 'u1', '2026-08-24T11:00:00Z', '2026-08-25T01:00:00Z');
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
            ('111111111111111111', 'A', ARRAY['{"key":0,"num":1,"type":"日前"}','{"key":1,"num":30,"type":"分前"}'],
             true,  '2026-08-26T00:00:00', '2026-08-26T00:00:00', '2026-08-25T09:00:00'),
            -- その前の 24 時間 (08-23 12:00 〜 08-24 12:00)
            ('111111111111111111', 'B', ARRAY[]::text[],
             false, '2026-08-26T10:00:00', '2026-08-26T11:00:00', '2026-08-24T06:00:00'),
            -- 直近 7 日
            ('111111111111111111', 'C', ARRAY['{"key":0,"num":1,"type":"時間前"}'],
             false, '2026-08-27T10:00:00', '2026-08-27T11:00:00', '2026-08-20T09:00:00'),
            -- その前の 7 日
            ('222222222222222222', 'D', ARRAY[]::text[],
             false, '2026-08-28T10:00:00', '2026-08-28T11:00:00', '2026-08-12T09:00:00'),
            -- 直近 30 日
            ('222222222222222222', 'E', ARRAY[]::text[],
             false, '2026-08-29T10:00:00', '2026-08-29T11:00:00', '2026-08-01T09:00:00'),
            ('333333333333333333', 'G', ARRAY[]::text[],
             false, '2026-08-30T10:00:00', '2026-08-30T11:00:00', '2026-08-05T09:00:00'),
            -- その前の 30 日 (日別 30 日の窓からは外れる)
            ('333333333333333333', 'F', ARRAY[]::text[],
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
            ('111111111111111111', 'X', ARRAY['garbage'],
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00'),
            -- 単位が未知
            ('111111111111111111', 'Y', ARRAY['{"key":0,"num":2,"type":"年前"}'],
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00'),
            -- num が負で u32 にできない
            ('111111111111111111', 'Z', ARRAY['{"key":0,"num":-1,"type":"分前"}'],
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
            ('111111111111111111', 'W', ARRAY['garbage', '{"key":1,"num":5,"type":"分前"}'],
             false, '2026-09-01T10:00:00', '2026-09-01T11:00:00', '2026-08-25T09:00:00');
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn active_users_count_sessions_overlapping_each_window(pool: PgPool) {
    seed_auth(&pool).await;

    let active = admin_analytics::active_users(&pool, now_utc())
        .await
        .unwrap();

    // 直近 1 日: u1 (s1 と s5 があるが DISTINCT で 1 人) と u2。その前は u1 と u4
    assert_eq!(active.daily.current, 2);
    assert_eq!(active.daily.previous, 2);
    assert_eq!(active.daily.delta, 0);
    assert_eq!(active.daily.change_percent, Some(0.0));

    // 直近 7 日: u1 / u2 / u4。その前は u1 だけ
    assert_eq!(active.weekly.current, 3);
    assert_eq!(active.weekly.previous, 1);

    // 直近 30 日: u1 / u2 / u4。その前は u3 だけ (07-10 で使うのをやめた利用者)
    assert_eq!(active.monthly.current, 3);
    assert_eq!(active.monthly.previous, 1);
    assert_eq!(active.monthly.delta, 2);
    assert_eq!(active.monthly.change_percent, Some(200.0));
}

/// 使い続けている利用者の行は `updatedAt` が最新に動くので、`updatedAt` が期間内にあるかどうかで
/// 数えると過去の期間から消えてしまう。生存期間の重なりで数えていることを確かめる
#[sqlx::test(migrations = "./migrations")]
async fn active_users_keep_counting_a_session_that_is_still_in_use(pool: PgPool) {
    sqlx::raw_sql(
        r#"
        CREATE TABLE "user" (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL,
            "emailVerified" BOOLEAN NOT NULL DEFAULT false, image TEXT,
            "createdAt" TIMESTAMPTZ NOT NULL DEFAULT now(), "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT now()
        );
        CREATE TABLE "session" (
            id TEXT PRIMARY KEY, "expiresAt" TIMESTAMPTZ NOT NULL, token TEXT NOT NULL,
            "createdAt" TIMESTAMPTZ NOT NULL DEFAULT now(), "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT now(),
            "ipAddress" TEXT, "userAgent" TEXT, "userId" TEXT NOT NULL
        );
        INSERT INTO "user" (id, name, email) VALUES ('u1', 'Alice', 'alice@example.com');
        -- 半年前からずっと使い続けている 1 行 (updatedAt は今日まで延び続けている)
        INSERT INTO "session" (id, "expiresAt", token, "userId", "createdAt", "updatedAt") VALUES
            ('s1', '2026-09-01T00:00:00Z', 't1', 'u1', '2026-03-01T00:00:00Z', '2026-08-25T02:00:00Z');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let active = admin_analytics::active_users(&pool, now_utc())
        .await
        .unwrap();
    // 直近の期間だけでなく、その前の期間でもアクティブとして数える
    assert_eq!(active.daily.current, 1);
    assert_eq!(active.daily.previous, 1);
    assert_eq!(active.monthly.current, 1);
    assert_eq!(active.monthly.previous, 1);

    let monthly = admin_analytics::monthly(&pool, now_utc()).await.unwrap();
    // 3 月に作られて今も生きているので、4 月以降のどの月でもアクティブとして数える
    let april = monthly
        .iter()
        .find(|m| m.month == date("2026-04-01"))
        .unwrap();
    assert_eq!(april.active_users, 1);
    // 作られる前の月は数えない
    let february = monthly
        .iter()
        .find(|m| m.month == date("2026-02-01"))
        .unwrap();
    assert_eq!(february.active_users, 0);
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

/// 配列の長さではなく `Notification::decode_all` が解釈できた数を数えていること。
/// 旧データや手動修正で壊れた設定が入っていても、api / Bot はそれを捨てて通知しない
#[sqlx::test(migrations = "./migrations")]
async fn notification_stats_count_only_settings_that_actually_notify(pool: PgPool) {
    seed_guilds(&pool).await;

    // 通知を設定しているのは A (2 件) と C (1 件)
    assert_eq!(
        admin_analytics::notification_stats(&pool).await.unwrap(),
        (2, 3)
    );

    seed_broken_notifications(&pool).await;
    // 壊れた JSON / 未知の単位 / 負の num しか持たない予定は数に入らない
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
}

#[sqlx::test(migrations = "./migrations")]
async fn daily_series_covers_every_day_including_empty_ones(pool: PgPool) {
    seed_auth(&pool).await;
    seed_guilds(&pool).await;

    let daily = admin_analytics::daily(&pool, now_jst().date())
        .await
        .unwrap();

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

    // 新規ユーザーとログインは JST の日付で分ける (u2 の 08-24 10:00Z は JST では 19:00)
    assert_eq!(on("2026-08-25").new_users, 1); // u1
    assert_eq!(on("2026-08-24").new_users, 1); // u2
    assert_eq!(on("2026-08-20").new_users, 1); // u4
    // s2 (08-24 19:00 JST) と s5 (08-24 20:00 JST) の 2 回
    assert_eq!(on("2026-08-24").logins, 2);
    assert_eq!(daily.iter().map(|p| p.logins).sum::<i64>(), 4);
}

#[sqlx::test(migrations = "./migrations")]
async fn monthly_series_buckets_by_jst_month(pool: PgPool) {
    seed_auth(&pool).await;
    seed_guilds(&pool).await;

    let monthly = admin_analytics::monthly(&pool, now_utc()).await.unwrap();

    assert_eq!(monthly.len() as i32, admin_analytics::MONTHLY_MONTHS);
    assert_eq!(monthly[0].month, date("2025-09-01"));
    assert_eq!(monthly[monthly.len() - 1].month, date("2026-08-01"));

    let on = |m: &str| monthly.iter().find(|p| p.month == date(m)).unwrap();
    let august = on("2026-08-01");
    assert_eq!(august.events, 6); // F 以外
    assert_eq!(august.new_users, 3); // u1 / u2 / u4
    assert_eq!(august.logins, 4); // s1 / s2 / s4 / s5
    assert_eq!(august.active_users, 3); // u1 / u2 / u4

    let july = on("2026-07-01");
    assert_eq!(july.events, 1); // F
    assert_eq!(july.new_users, 1); // u3
    assert_eq!(july.active_users, 1); // u3 (07-10 まで使っていた)
}

#[sqlx::test(migrations = "./migrations")]
async fn guild_activity_ranks_guilds_by_recent_events(pool: PgPool) {
    seed_guilds(&pool).await;

    let (active_guilds, joined_guilds) = admin_analytics::guild_counts(&pool, recent_since())
        .await
        .unwrap();

    // 直近 30 日に予定が作られたのは A (3 件) / B (2 件) / 退出済み (1 件)。予定なしサーバーは入らない
    assert_eq!(active_guilds, 3);
    assert_eq!(joined_guilds, 3);

    let top = admin_analytics::top_guilds(&pool, recent_since())
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
