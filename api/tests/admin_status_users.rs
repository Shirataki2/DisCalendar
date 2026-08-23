//! 管理コンソールの概要・稼働状況・ユーザー / セッション・監査ログ (#37) の DB テスト。
//! `#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する (Postgres が必要)。
//! Better Auth のテーブル (`user` / `session` / `account`) はマイグレーションに含まれない
//! (web の Better Auth が作る) ので、ここで本番と同じ形に作る。

use chrono::NaiveDateTime;
use discalendar_api::{
    auth::AuthUser,
    models::{
        admin_audit::{self, AuditEntry},
        admin_stats, admin_status, admin_users,
    },
};
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const LEFT_GUILD: &str = "222222222222222222";

fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}

/// Better Auth が作るテーブルを本番と同じ列で用意する (web/src/lib/auth.ts の betterAuth が作る形)
async fn create_auth_tables(pool: &PgPool) {
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
        CREATE TABLE "account" (
            id TEXT PRIMARY KEY, issuer TEXT NOT NULL DEFAULT 'discord', "accountId" TEXT NOT NULL,
            "providerId" TEXT NOT NULL, "userId" TEXT NOT NULL,
            "accessToken" TEXT, "refreshToken" TEXT,
            "createdAt" TIMESTAMPTZ NOT NULL DEFAULT now(), "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT now()
        );
        INSERT INTO "user" (id, name, email, "createdAt") VALUES
            ('u1', 'Alice', 'alice@example.com', now() - interval '2 days'),
            ('u2', 'Bob', 'bob@example.com', now() - interval '1 day');
        INSERT INTO "account" (id, "accountId", "providerId", "userId") VALUES
            ('a1', '123456789012345678', 'discord', 'u1');
        INSERT INTO "session" (id, "expiresAt", token, "userId", "ipAddress", "userAgent") VALUES
            ('s1', now() + interval '1 day', 'token-1', 'u1', '203.0.113.1', 'Firefox'),
            ('s2', now() - interval '1 day', 'token-2', 'u1', NULL, NULL),
            ('s3', now() + interval '1 day', 'token-3', 'u2', NULL, NULL);
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

/// ギルド・予定を入れる (退出済みギルド LEFT_GUILD には予定だけ残す)。
/// GUILD には通知先チャンネルを設定しておく (未設定のギルドには Bot が通知を送らないため)
async fn seed_guilds(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        INSERT INTO guilds (guild_id, name) VALUES ('111111111111111111', 'メインサーバー');
        INSERT INTO event_settings (guild_id, channel_id) VALUES ('111111111111111111', '444444444444444444');
        INSERT INTO events (guild_id, name, notifications, start_at, end_at) VALUES
            ('111111111111111111', '定例', ARRAY['{"key":0,"num":1,"type":"日前"}'],
             '2026-08-24T10:00:00', '2026-08-24T11:00:00'),
            ('111111111111111111', '終わった予定', ARRAY[]::text[],
             '2020-01-01T10:00:00', '2020-01-01T11:00:00'),
            ('222222222222222222', '残骸', ARRAY[]::text[],
             '2026-09-01T10:00:00', '2026-09-01T11:00:00');
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn stats_count_guilds_events_and_users(pool: PgPool) {
    create_auth_tables(&pool).await;
    seed_guilds(&pool).await;

    let counts = admin_stats::counts(&pool, dt("2026-08-23T00:00:00"))
        .await
        .unwrap();
    assert_eq!(counts.guilds, 1);
    // 退出済みギルド (予定だけ残っている) も痕跡として数える
    assert_eq!(counts.known_guilds, 2);
    assert_eq!(counts.left_guilds, 1);
    assert_eq!(counts.events, 3);
    // 2020 年の予定は終わっている
    assert_eq!(counts.upcoming_events, 2);
    assert_eq!(counts.users, 2);
    assert_eq!(counts.active_sessions, 2);
    assert_eq!(counts.sessions, 3);
}

#[sqlx::test(migrations = "./migrations")]
async fn stats_list_recent_and_left_guilds(pool: PgPool) {
    seed_guilds(&pool).await;

    let recent = admin_stats::recent_guilds(&pool).await.unwrap();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].guild_id, GUILD);

    let left = admin_stats::left_guilds(&pool).await.unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].guild_id, LEFT_GUILD);
    assert_eq!(left[0].event_count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn counts_notifications_firing_today(pool: PgPool) {
    seed_guilds(&pool).await;

    // 8/24 10:00 開始の「1 日前」通知は 8/23 10:00 に飛ぶ
    let today = admin_stats::notifications_between(
        &pool,
        dt("2026-08-23T00:00:00"),
        dt("2026-08-24T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(today, 1);

    // 翌日は同じ予定の開始時刻通知 (Bot が必ず送る 0 分前) が飛ぶ。
    // 9/1 の予定 (LEFT_GUILD) は通知先チャンネルが無いので数に入らない
    let tomorrow = admin_stats::notifications_between(
        &pool,
        dt("2026-08-24T00:00:00"),
        dt("2026-08-25T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(tomorrow, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn upcoming_events_keeps_all_day_events_until_the_next_midnight(pool: PgPool) {
    create_auth_tables(&pool).await;
    // 終日予定の end_at は「終了日 (含む) の 0:00」で保存される (web の toApiRange)
    sqlx::raw_sql(
        r#"
        INSERT INTO events (guild_id, name, notifications, is_all_day, start_at, end_at) VALUES
            ('111111111111111111', '今日の終日予定', ARRAY[]::text[], true,
             '2026-08-23T00:00:00', '2026-08-23T00:00:00');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // その日のうちはまだ終わっていない
    let during = admin_stats::counts(&pool, dt("2026-08-23T09:00:00"))
        .await
        .unwrap();
    assert_eq!(during.upcoming_events, 1);
    // 翌日 0:00 で終了扱い
    let after = admin_stats::counts(&pool, dt("2026-08-24T00:00:00"))
        .await
        .unwrap();
    assert_eq!(after.upcoming_events, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn does_not_count_notifications_for_left_guilds(pool: PgPool) {
    // Bot の退出処理は guilds の行だけを消すので、通知先の設定と予定は残る。
    // ただし送信は Missing Access になって届かないので数えない
    sqlx::raw_sql(
        r#"
        INSERT INTO event_settings (guild_id, channel_id) VALUES ('222222222222222222', '555555555555555555');
        INSERT INTO events (guild_id, name, notifications, start_at, end_at) VALUES
            ('222222222222222222', '退出済みの予定', ARRAY[]::text[],
             '2026-08-23T10:00:00', '2026-08-23T11:00:00');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let count = admin_stats::notifications_between(
        &pool,
        dt("2026-08-23T00:00:00"),
        dt("2026-08-24T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn counts_notifications_set_further_ahead_than_a_year(pool: PgPool) {
    // web のフォームが許す最大の事前通知 (100 週間前 = 700 日前)
    sqlx::raw_sql(
        r#"
        INSERT INTO guilds (guild_id, name) VALUES ('111111111111111111', 'メインサーバー');
        INSERT INTO event_settings (guild_id, channel_id) VALUES ('111111111111111111', '444444444444444444');
        INSERT INTO events (guild_id, name, notifications, start_at, end_at) VALUES
            ('111111111111111111', '2 年近く先の予定', ARRAY['{"key":0,"num":100,"type":"週間前"}'],
             '2028-07-23T10:00:00', '2028-07-23T11:00:00');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // 2028-07-23 10:00 の 700 日前 = 2026-08-23 10:00
    let count = admin_stats::notifications_between(
        &pool,
        dt("2026-08-23T00:00:00"),
        dt("2026-08-24T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn does_not_count_notifications_for_invalid_channels(pool: PgPool) {
    // Bot は channel_id を NonZeroU64 として検証し、不正なら何も送らない
    sqlx::raw_sql(
        r#"
        INSERT INTO guilds (guild_id, name) VALUES ('444444444444444444', '不正な通知先');
        INSERT INTO event_settings (guild_id, channel_id) VALUES ('444444444444444444', '0');
        INSERT INTO events (guild_id, name, notifications, start_at, end_at) VALUES
            ('444444444444444444', '通知先が不正', ARRAY[]::text[],
             '2026-08-23T10:00:00', '2026-08-23T11:00:00');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let count = admin_stats::notifications_between(
        &pool,
        dt("2026-08-23T00:00:00"),
        dt("2026-08-24T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn does_not_count_notifications_for_guilds_without_a_channel(pool: PgPool) {
    // 通知先チャンネルを設定していないギルドの予定 (Bot は event_settings が無いと何も送らない)
    sqlx::raw_sql(
        r#"
        INSERT INTO guilds (guild_id, name) VALUES ('333333333333333333', '未設定サーバー');
        INSERT INTO events (guild_id, name, notifications, start_at, end_at) VALUES
            ('333333333333333333', '通知先未設定', ARRAY['{"key":0,"num":30,"type":"分前"}'],
             '2026-08-23T10:00:00', '2026-08-23T11:00:00');
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let count = admin_stats::notifications_between(
        &pool,
        dt("2026-08-23T00:00:00"),
        dt("2026-08-24T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn migration_status_is_clean_after_migrating(pool: PgPool) {
    let status = admin_status::migration_status(&pool).await.unwrap();
    assert!(!status.table_missing);
    assert!(status.applied_count > 0);
    assert!(status.pending.is_empty(), "未適用: {:?}", status.pending);
    assert!(status.failed.is_empty());
    assert!(
        status.checksum_mismatch.is_empty(),
        "既存のマイグレーションファイルを変更してはいけない (AGENTS.md の P0)"
    );
    assert!(status.unknown.is_empty());
    assert!(!status.has_problem());
    assert!(status.latest.is_some());
}

#[sqlx::test(migrations = "./migrations")]
async fn users_are_searchable_and_do_not_expose_tokens(pool: PgPool) {
    create_auth_tables(&pool).await;

    let all = admin_users::list(&pool, "", 1).await.unwrap();
    assert_eq!(all.len(), 2);
    // 新規登録が新しい順
    assert_eq!(all[0].id, "u2");
    assert_eq!(admin_users::count(&pool, "").await.unwrap(), 2);

    // 名前の部分一致 (大文字小文字を区別しない)
    let by_name = admin_users::list(&pool, "ali", 1).await.unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].id, "u1");
    assert_eq!(
        by_name[0].discord_user_id.as_deref(),
        Some("123456789012345678")
    );
    assert_eq!(by_name[0].active_sessions, 1);
    assert_eq!(by_name[0].sessions, 2);
    assert!(by_name[0].last_session_at.is_some());

    // メールアドレスの部分一致と Discord ユーザー ID の完全一致でも引ける
    assert_eq!(admin_users::count(&pool, "bob@").await.unwrap(), 1);
    assert_eq!(
        admin_users::count(&pool, "123456789012345678")
            .await
            .unwrap(),
        1
    );
    // 部分一致のメタ文字はリテラル扱い (全件返らない)
    assert_eq!(admin_users::count(&pool, "%").await.unwrap(), 0);

    let sessions = admin_users::sessions(&pool, "u1").await.unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions.iter().filter(|s| s.expired).count(), 1);
    // トークンは型として存在しない (SessionSummary に token は無い) ことを JSON でも確かめる
    let json = serde_json::to_string(&sessions).unwrap();
    assert!(
        !json.contains("token-1"),
        "セッショントークンを返している: {json}"
    );
    assert!(
        !json.contains("token"),
        "token を含むフィールドがある: {json}"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn revoking_sessions_only_affects_that_user(pool: PgPool) {
    create_auth_tables(&pool).await;

    assert!(admin_users::exists(&pool, "u1").await.unwrap());
    assert!(!admin_users::exists(&pool, "nobody").await.unwrap());

    // 期限切れも含めて消す
    assert_eq!(admin_users::delete_sessions(&pool, "u1").await.unwrap(), 2);
    assert!(admin_users::sessions(&pool, "u1").await.unwrap().is_empty());
    // 他のユーザーのセッションは残る
    assert_eq!(admin_users::sessions(&pool, "u2").await.unwrap().len(), 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn audit_logs_are_listed_newest_first_and_filterable(pool: PgPool) {
    let alice = AuthUser {
        id: "u1".to_owned(),
        name: "Alice".to_owned(),
        discord_user_id: "123456789012345678".to_owned(),
    };
    let bob = AuthUser {
        id: "u2".to_owned(),
        name: "Bob".to_owned(),
        discord_user_id: "987654321098765432".to_owned(),
    };
    for (actor, action) in [
        (&alice, "event.update"),
        (&bob, "sql.select"),
        (&alice, "user.revoke_sessions"),
    ] {
        admin_audit::record(
            &pool,
            actor,
            AuditEntry {
                action,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    let all = admin_audit::list(&pool, "", "", 1).await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].action, "user.revoke_sessions");
    assert_eq!(admin_audit::count(&pool, "", "").await.unwrap(), 3);

    // action / actor で絞れる
    assert_eq!(
        admin_audit::list(&pool, "sql.select", "", 1)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        admin_audit::count(&pool, "", "123456789012345678")
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        admin_audit::count(&pool, "event.update", "987654321098765432")
            .await
            .unwrap(),
        0
    );

    let actions = admin_audit::actions(&pool).await.unwrap();
    assert_eq!(
        actions,
        vec!["event.update", "sql.select", "user.revoke_sessions"]
    );
}
