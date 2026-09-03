//! DB を使う統合テスト。`#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する。
//! 実行には `DATABASE_URL` (api/.env でも可) で接続できる Postgres が必要。

use chrono::NaiveDateTime;
use discalendar_api::models::{
    event_links,
    events::{self, EventInput},
    feed_tokens, guilds,
    notifications::{Notification, NotificationUnit},
};
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";

fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}

fn input(name: &str, start: &str, end: &str) -> EventInput {
    EventInput {
        name: name.to_owned(),
        description: Some("desc".to_owned()),
        notifications: vec![Notification {
            num: 30,
            unit: NotificationUnit::Minutes,
        }],
        color: "#2196F3".to_owned(),
        is_all_day: false,
        start_at: dt(start),
        end_at: dt(end),
        discord_scheduled_event: None,
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn event_crud_is_scoped_to_guild(pool: PgPool) {
    let created = events::create(
        &pool,
        GUILD,
        &input("meeting", "2026-08-22T10:00:00", "2026-08-22T11:00:00"),
        dt("2026-08-01T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(created.guild_id, GUILD);
    // 通知は API の入出力と同じ形で保存される
    assert_eq!(
        created.notifications,
        serde_json::json!([{ "num": 30, "unit": "minutes" }])
    );

    // 他ギルドからは更新・削除できない
    let mut renamed = input("renamed", "2026-08-22T10:00:00", "2026-08-22T12:00:00");
    renamed.notifications.clear();
    assert!(
        events::update(&pool, OTHER_GUILD, created.id, &renamed)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !events::delete(&pool, OTHER_GUILD, created.id)
            .await
            .unwrap()
    );

    let updated = events::update(&pool, GUILD, created.id, &renamed)
        .await
        .unwrap()
        .expect("event exists");
    assert_eq!(updated.name, "renamed");
    assert_eq!(updated.end_at, dt("2026-08-22T12:00:00"));
    assert_eq!(updated.notifications, serde_json::json!([]));
    // created_at は更新で変わらない
    assert_eq!(updated.created_at, created.created_at);

    assert!(events::delete(&pool, GUILD, created.id).await.unwrap());
    assert!(!events::delete(&pool, GUILD, created.id).await.unwrap());
}

#[sqlx::test(migrations = "./migrations")]
async fn list_returns_events_overlapping_range(pool: PgPool) {
    let now = dt("2026-08-01T00:00:00");
    // 範囲より前に始まり範囲内で終わる
    events::create(
        &pool,
        GUILD,
        &input("spans-into", "2026-08-30T10:00:00", "2026-09-02T10:00:00"),
        now,
    )
    .await
    .unwrap();
    // 範囲内
    events::create(
        &pool,
        GUILD,
        &input("inside", "2026-09-10T10:00:00", "2026-09-10T11:00:00"),
        now,
    )
    .await
    .unwrap();
    // 範囲の終端ちょうどに始まる (end は含まない)
    events::create(
        &pool,
        GUILD,
        &input("at-end", "2026-10-01T00:00:00", "2026-10-01T01:00:00"),
        now,
    )
    .await
    .unwrap();
    // 範囲より前
    events::create(
        &pool,
        GUILD,
        &input("before", "2026-08-01T10:00:00", "2026-08-01T11:00:00"),
        now,
    )
    .await
    .unwrap();
    // 他ギルド
    events::create(
        &pool,
        OTHER_GUILD,
        &input("other", "2026-09-10T10:00:00", "2026-09-10T11:00:00"),
        now,
    )
    .await
    .unwrap();

    let rows = events::list_between(
        &pool,
        GUILD,
        dt("2026-09-01T00:00:00"),
        dt("2026-10-01T00:00:00"),
    )
    .await
    .unwrap();
    let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["spans-into", "inside"]);
}

/// 横断カレンダー (#98) の複数ギルド版一覧。指定したギルドだけを跨いで返し、並びは開始日時順
#[sqlx::test(migrations = "./migrations")]
async fn list_between_guilds_spans_guilds_and_excludes_others(pool: PgPool) {
    const THIRD_GUILD: &str = "333333333333333333";
    let now = dt("2026-08-01T00:00:00");
    for (guild, name, start, end) in [
        (
            OTHER_GUILD,
            "other-first",
            "2026-09-05T10:00:00",
            "2026-09-05T11:00:00",
        ),
        (GUILD, "mine", "2026-09-10T10:00:00", "2026-09-10T11:00:00"),
        (
            THIRD_GUILD,
            "third",
            "2026-09-10T10:00:00",
            "2026-09-10T11:00:00",
        ),
        (
            GUILD,
            "mine-outside",
            "2026-10-10T10:00:00",
            "2026-10-10T11:00:00",
        ),
    ] {
        events::create(&pool, guild, &input(name, start, end), now)
            .await
            .unwrap();
    }

    let range = (dt("2026-09-01T00:00:00"), dt("2026-10-01T00:00:00"));
    let rows = events::list_between_guilds(
        &pool,
        &[GUILD.to_owned(), OTHER_GUILD.to_owned()],
        range.0,
        range.1,
    )
    .await
    .unwrap();
    let names: Vec<_> = rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["other-first", "mine"]);
    assert_eq!(rows[0].guild_id, OTHER_GUILD);
    assert_eq!(rows[1].guild_id, GUILD);

    // 指定していないギルドは 0 件、空指定も 0 件
    let rows = events::list_between_guilds(&pool, &[THIRD_GUILD.to_owned()], range.0, range.1)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "third");
    assert!(
        events::list_between_guilds(&pool, &[], range.0, range.1)
            .await
            .unwrap()
            .is_empty()
    );
}

/// Discord スケジュールイベントとの対応付け (`event_discord_links`、#94)
#[sqlx::test(migrations = "./migrations")]
async fn event_links_are_scoped_and_cascade(pool: PgPool) {
    let now = dt("2026-08-01T00:00:00");
    let event = events::create(
        &pool,
        GUILD,
        &input("linked", "2026-09-10T10:00:00", "2026-09-10T11:00:00"),
        now,
    )
    .await
    .unwrap();
    // 作成直後は未連携
    assert_eq!(event.discord_scheduled_event_id, None);

    event_links::insert(&pool, GUILD, event.id, "9001", now)
        .await
        .unwrap();

    // 一覧・単体取得に JOIN で載る
    let rows = events::list_between(
        &pool,
        GUILD,
        dt("2026-09-01T00:00:00"),
        dt("2026-10-01T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(rows[0].discord_scheduled_event_id.as_deref(), Some("9001"));
    let mut tx = pool.begin().await.unwrap();
    let found = events::find_by_id_for_update(&mut tx, GUILD, event.id)
        .await
        .unwrap()
        .expect("event exists");
    assert_eq!(found.discord_scheduled_event_id.as_deref(), Some("9001"));
    tx.commit().await.unwrap();

    // 他ギルドからは読めない・消せない・差し替えられない
    assert_eq!(
        event_links::get(&pool, OTHER_GUILD, event.id)
            .await
            .unwrap(),
        None
    );
    assert!(
        !event_links::delete(&pool, OTHER_GUILD, event.id)
            .await
            .unwrap()
    );
    event_links::set_scheduled_event_id(&pool, OTHER_GUILD, event.id, "9002")
        .await
        .unwrap();
    assert_eq!(
        event_links::get(&pool, GUILD, event.id)
            .await
            .unwrap()
            .as_deref(),
        Some("9001")
    );

    // 差し替え (Discord 側で手動削除されたイベントの作り直し)
    event_links::set_scheduled_event_id(&pool, GUILD, event.id, "9003")
        .await
        .unwrap();
    assert_eq!(
        event_links::get(&pool, GUILD, event.id)
            .await
            .unwrap()
            .as_deref(),
        Some("9003")
    );

    // 予定の削除に CASCADE で追随する
    assert!(events::delete(&pool, GUILD, event.id).await.unwrap());
    assert_eq!(
        event_links::get(&pool, GUILD, event.id).await.unwrap(),
        None
    );
}

/// 一括削除用の一覧はギルドで絞る (#94)。開始日時では絞らない:
/// DB の `start_at` は Discord に同期していない変更 (管理コンソールの編集) でずれることがあり、
/// 「開始前だけ消す」判定に使うと Discord 側に未来のイベントを取り残してしまう
#[sqlx::test(migrations = "./migrations")]
async fn scheduled_event_ids_are_filtered_by_guild(pool: PgPool) {
    let now = dt("2026-08-01T00:00:00");
    let future = events::create(
        &pool,
        GUILD,
        &input("future", "2026-09-10T10:00:00", "2026-09-10T11:00:00"),
        now,
    )
    .await
    .unwrap();
    let past = events::create(
        &pool,
        GUILD,
        &input("past", "2026-07-10T10:00:00", "2026-07-10T11:00:00"),
        now,
    )
    .await
    .unwrap();
    let other = events::create(
        &pool,
        OTHER_GUILD,
        &input("other", "2026-09-10T10:00:00", "2026-09-10T11:00:00"),
        now,
    )
    .await
    .unwrap();
    event_links::insert(&pool, GUILD, future.id, "1", now)
        .await
        .unwrap();
    event_links::insert(&pool, GUILD, past.id, "2", now)
        .await
        .unwrap();
    event_links::insert(&pool, OTHER_GUILD, other.id, "3", now)
        .await
        .unwrap();

    let mut ids = event_links::list_scheduled_event_ids(&pool, GUILD)
        .await
        .unwrap();
    ids.sort();
    // 開始が過去の予定 ("2") も含む。他ギルドの予定 ("3") は含まない
    assert_eq!(ids, vec!["1".to_owned(), "2".to_owned()]);
}

/// `notifications` の中身は DB では検証していない (CHECK は配列であることだけ) ので、
/// 手作業などで PostgreSQL では有効な JSONB が入りうる。f64 に収まらない数値が 1 つあるだけで
/// `serde_json::Value` へのデコードごと失敗すると、その予定だけでなく一覧全体
/// (Bot の通知タスクなら全ギルドの通知) が止まる。
/// workspace の serde_json は `arbitrary_precision` を有効にしてあるので、まず値として読めて、
/// 解釈できない要素だけが `Notification::decode_all` で落ちる
#[sqlx::test(migrations = "./migrations")]
async fn events_with_out_of_range_numbers_in_notifications_are_still_readable(pool: PgPool) {
    sqlx::query(
        r#"
        INSERT INTO events (guild_id, name, notifications, color, is_all_day, start_at, end_at, created_at)
        VALUES ($1, '桁が大きすぎる通知', '[{"num":1e400,"unit":"minutes"},{"num":30,"unit":"minutes"}]'::jsonb,
                '#2196F3', false, '2026-09-05 10:00:00', '2026-09-05 11:00:00', '2026-09-01 00:00:00')
        "#,
    )
    .bind(GUILD)
    .execute(&pool)
    .await
    .unwrap();

    let rows = events::list_between(
        &pool,
        GUILD,
        dt("2026-09-01T00:00:00"),
        dt("2026-10-01T00:00:00"),
    )
    .await
    .expect("a row with an out-of-range number must not break the whole query");
    assert_eq!(rows.len(), 1);
    // 読めない要素だけが無視され、残りの通知は使える
    assert_eq!(
        Notification::decode_all(&rows[0].notifications),
        vec![Notification {
            num: 30,
            unit: NotificationUnit::Minutes
        }]
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn guild_config_defaults_and_upserts(pool: PgPool) {
    let config = guilds::get_config(&pool, GUILD).await.unwrap();
    assert!(!config.restricted);
    assert_eq!(config.guild_id, GUILD);

    let config = guilds::upsert_config(&pool, GUILD, true).await.unwrap();
    assert!(config.restricted);
    assert!(guilds::get_config(&pool, GUILD).await.unwrap().restricted);

    let config = guilds::upsert_config(&pool, GUILD, false).await.unwrap();
    assert!(!config.restricted);
}

#[sqlx::test(migrations = "./migrations")]
async fn lock_config_for_update_creates_default_row_inside_transaction(pool: PgPool) {
    // 行が無いギルド: 既定値の行を確保して返す (ロールバックすれば残らない)
    let mut tx = pool.begin().await.unwrap();
    let before = guilds::lock_config_for_update(&mut tx, GUILD)
        .await
        .unwrap();
    assert!(!before.restricted);
    tx.rollback().await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM guild_config WHERE guild_id = $1")
        .bind(GUILD)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);

    // 既に行があれば、その値を返して上書きしない
    guilds::upsert_config(&pool, GUILD, true).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let before = guilds::lock_config_for_update(&mut tx, GUILD)
        .await
        .unwrap();
    assert!(before.restricted);
    let after = guilds::upsert_config(&mut *tx, GUILD, false).await.unwrap();
    assert!(!after.restricted);
    tx.commit().await.unwrap();
    assert!(!guilds::get_config(&pool, GUILD).await.unwrap().restricted);
}

#[sqlx::test(migrations = "./migrations")]
async fn joined_guilds_filters_by_bot_registry(pool: PgPool) {
    sqlx::query(
        "INSERT INTO guilds (guild_id, name, avatar_url, locale) VALUES ($1, 'A', NULL, 'ja')",
    )
    .bind(GUILD)
    .execute(&pool)
    .await
    .unwrap();

    let joined = guilds::find_joined(&pool, &[GUILD.to_owned(), OTHER_GUILD.to_owned()])
        .await
        .unwrap();
    assert_eq!(joined.len(), 1);
    assert_eq!(joined[0].guild_id, GUILD);
    assert_eq!(joined[0].name, "A");

    assert!(
        guilds::find_by_guild_id(&pool, OTHER_GUILD)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_audit_log_is_recorded(pool: PgPool) {
    use discalendar_api::{
        auth::AuthUser,
        models::admin_audit::{self, AuditEntry},
    };
    use serde_json::json;

    let actor = AuthUser {
        id: "user_1".to_owned(),
        name: "admin".to_owned(),
        discord_user_id: "123456789012345678".to_owned(),
    };
    let entry = AuditEntry {
        action: "event.update",
        target_type: Some("event"),
        target_id: Some("42"),
        before: Some(json!({ "name": "old" })),
        after: Some(json!({ "name": "new" })),
        detail: None,
    };
    let log = admin_audit::record(&pool, &actor, entry).await.unwrap();
    assert_eq!(log.actor_user_id, "user_1");
    assert_eq!(log.actor_discord_user_id, "123456789012345678");
    assert_eq!(log.action, "event.update");
    assert_eq!(log.target_type.as_deref(), Some("event"));
    assert_eq!(log.target_id.as_deref(), Some("42"));
    assert_eq!(log.before, Some(json!({ "name": "old" })));
    assert_eq!(log.after, Some(json!({ "name": "new" })));
    assert!(log.detail.is_none());

    // 対象なし・スナップショットなし (SQL 実行など) でも書ける
    let log = admin_audit::record(
        &pool,
        &actor,
        AuditEntry {
            action: "sql.select",
            detail: Some(json!({ "sql": "SELECT 1", "rows": 1 })),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(log.target_type.is_none());
    assert_eq!(log.detail, Some(json!({ "sql": "SELECT 1", "rows": 1 })));

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM admin_audit_logs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[sqlx::test(migrations = "./migrations")]
async fn admin_guild_list_joins_config_settings_and_counts(pool: PgPool) {
    use discalendar_api::models::admin_guilds;

    for (id, name) in [(GUILD, "Alpha Guild"), (OTHER_GUILD, "beta_server")] {
        sqlx::query(
            "INSERT INTO guilds (guild_id, name, avatar_url, locale) VALUES ($1, $2, NULL, 'ja')",
        )
        .bind(id)
        .bind(name)
        .execute(&pool)
        .await
        .unwrap();
    }
    guilds::upsert_config(&pool, GUILD, true).await.unwrap();
    sqlx::query(
        "INSERT INTO event_settings (guild_id, channel_id) VALUES ($1, '333333333333333333')",
    )
    .bind(GUILD)
    .execute(&pool)
    .await
    .unwrap();
    for i in 0..3 {
        events::create(
            &pool,
            GUILD,
            &input(
                &format!("e{i}"),
                "2026-08-22T10:00:00",
                "2026-08-22T11:00:00",
            ),
            dt("2026-08-01T00:00:00"),
        )
        .await
        .unwrap();
    }

    // Bot が退出して guilds の行が消えたが予定だけ残っているギルドも一覧に出る (名前なし、末尾)
    const LEFT_GUILD: &str = "444444444444444444";
    events::create(
        &pool,
        LEFT_GUILD,
        &input("orphan", "2026-08-22T10:00:00", "2026-08-22T11:00:00"),
        dt("2026-08-01T00:00:00"),
    )
    .await
    .unwrap();

    // 全件 (名前順、名前なしは末尾)
    let all = admin_guilds::list(&pool, "", 1).await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].guild_id, GUILD);
    assert_eq!(all[0].name.as_deref(), Some("Alpha Guild"));
    assert!(all[0].registered);
    assert!(all[0].restricted);
    assert_eq!(all[0].channel_id.as_deref(), Some("333333333333333333"));
    assert_eq!(all[0].event_count, 3);
    assert!(!all[1].restricted);
    assert!(all[1].channel_id.is_none());
    assert_eq!(all[1].event_count, 0);
    assert_eq!(all[2].guild_id, LEFT_GUILD);
    assert!(all[2].name.is_none());
    assert!(!all[2].registered);
    assert_eq!(all[2].event_count, 1);
    assert_eq!(admin_guilds::count(&pool, "").await.unwrap(), 3);

    // 名前の部分一致 (大文字小文字を区別しない) と guild_id の完全一致
    let by_name = admin_guilds::list(&pool, "ALPHA", 1).await.unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].guild_id, GUILD);
    let by_id = admin_guilds::list(&pool, OTHER_GUILD, 1).await.unwrap();
    assert_eq!(by_id.len(), 1);
    assert_eq!(by_id[0].name.as_deref(), Some("beta_server"));
    // LIKE のメタ文字はリテラル扱い ("_" が任意の 1 文字にならない)
    assert_eq!(admin_guilds::count(&pool, "a_s").await.unwrap(), 1);
    assert_eq!(admin_guilds::count(&pool, "a%s").await.unwrap(), 0);
    assert_eq!(admin_guilds::count(&pool, "nothing").await.unwrap(), 0);

    // 退出済みギルドは ID で引ける
    assert_eq!(admin_guilds::count(&pool, LEFT_GUILD).await.unwrap(), 1);

    // 2 ページ目は空
    assert!(admin_guilds::list(&pool, "", 2).await.unwrap().is_empty());

    let detail = admin_guilds::find(&pool, GUILD).await.unwrap().unwrap();
    assert_eq!(detail.event_count, 3);
    assert!(detail.registered);
    let left = admin_guilds::find(&pool, LEFT_GUILD)
        .await
        .unwrap()
        .unwrap();
    assert!(!left.registered);
    assert!(left.name.is_none());
    assert_eq!(left.event_count, 1);
    assert!(
        admin_guilds::find(&pool, "999999999999999999")
            .await
            .unwrap()
            .is_none()
    );

    // 書き込み前の存在確認: どれかのテーブルに行があれば true
    assert!(admin_guilds::exists(&pool, GUILD).await.unwrap());
    assert!(admin_guilds::exists(&pool, LEFT_GUILD).await.unwrap());
    assert!(
        !admin_guilds::exists(&pool, "999999999999999999")
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn event_writes_work_inside_a_transaction(pool: PgPool) {
    // 管理コンソールは監査ログと同じトランザクションで書くので、executor 経由でも動くことを確認する
    let mut tx = pool.begin().await.unwrap();
    let created = events::create(
        &mut *tx,
        GUILD,
        &input("tx", "2026-08-22T10:00:00", "2026-08-22T11:00:00"),
        dt("2026-08-01T00:00:00"),
    )
    .await
    .unwrap();
    let found = events::find_by_id_for_update(&mut tx, GUILD, created.id)
        .await
        .unwrap();
    assert_eq!(found.map(|e| e.name).as_deref(), Some("tx"));
    // 他ギルドの ID では見えない
    assert!(
        events::find_by_id_for_update(&mut tx, OTHER_GUILD, created.id)
            .await
            .unwrap()
            .is_none()
    );
    tx.rollback().await.unwrap();

    // ロールバックしたので残っていない
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        events::find_by_id_for_update(&mut conn, GUILD, created.id)
            .await
            .unwrap()
            .is_none()
    );
}

// iCal フィードのトークン (#95)

/// フィードの配信条件は「Bot が参加中 (guilds に行がある)」なので、テスト DB にギルドの行を入れる
async fn insert_guild(pool: &PgPool, guild_id: &str, name: &str) {
    sqlx::query(
        "INSERT INTO guilds (guild_id, name, avatar_url, locale) VALUES ($1, $2, NULL, 'ja')",
    )
    .bind(guild_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test(migrations = "./migrations")]
async fn feed_token_issue_rotate_and_revoke(pool: PgPool) {
    insert_guild(&pool, GUILD, "guild").await;
    assert!(feed_tokens::get(&pool, GUILD).await.unwrap().is_none());

    // 発行
    let first = feed_tokens::generate_token();
    let issued = feed_tokens::upsert(
        &pool,
        GUILD,
        &first,
        "100000000000000001",
        dt("2026-09-01T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(issued.token, first);
    assert_eq!(issued.created_by, "100000000000000001");
    assert_eq!(
        feed_tokens::get(&pool, GUILD)
            .await
            .unwrap()
            .map(|t| t.token),
        Some(first.clone())
    );
    assert_eq!(
        feed_tokens::find_guild_by_token(&pool, &first)
            .await
            .unwrap()
            .map(|g| g.guild_id),
        Some(GUILD.to_owned())
    );

    // 再発行で置き換わり、古いトークンは引けなくなる (1 ギルド 1 本)
    let second = feed_tokens::generate_token();
    let rotated = feed_tokens::upsert(
        &pool,
        GUILD,
        &second,
        "100000000000000002",
        dt("2026-09-02T00:00:00"),
    )
    .await
    .unwrap();
    assert_eq!(rotated.token, second);
    assert_eq!(rotated.created_by, "100000000000000002");
    assert!(
        feed_tokens::find_guild_by_token(&pool, &first)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        feed_tokens::find_guild_by_token(&pool, &second)
            .await
            .unwrap()
            .is_some()
    );

    // 無効化
    assert!(feed_tokens::delete(&pool, GUILD).await.unwrap());
    assert!(!feed_tokens::delete(&pool, GUILD).await.unwrap());
    assert!(feed_tokens::get(&pool, GUILD).await.unwrap().is_none());
    assert!(
        feed_tokens::find_guild_by_token(&pool, &second)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn feed_token_does_not_resolve_when_bot_has_left(pool: PgPool) {
    // guilds に行が無い (Bot が退出した / まだ書かれていない) ギルドのトークンは配信に使えない
    let token = feed_tokens::generate_token();
    feed_tokens::upsert(
        &pool,
        GUILD,
        &token,
        "100000000000000001",
        dt("2026-09-01T00:00:00"),
    )
    .await
    .unwrap();
    assert!(feed_tokens::get(&pool, GUILD).await.unwrap().is_some());
    assert!(
        feed_tokens::find_guild_by_token(&pool, &token)
            .await
            .unwrap()
            .is_none()
    );

    // 参加 (行が入る) すれば引ける
    insert_guild(&pool, GUILD, "guild").await;
    assert!(
        feed_tokens::find_guild_by_token(&pool, &token)
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn feed_lists_events_ending_after_the_cutoff(pool: PgPool) {
    let created_at = dt("2026-01-01T00:00:00");
    for (name, start, end) in [
        // 下限より前に終わった予定は含めない
        ("old", "2025-08-01T10:00:00", "2025-08-01T11:00:00"),
        // 下限ちょうどに終わる予定は含める (end_at >= since)
        ("edge", "2025-09-01T09:00:00", "2025-09-01T10:00:00"),
        // 下限をまたいで続いている予定は含める
        ("spanning", "2025-08-30T00:00:00", "2025-09-02T00:00:00"),
        ("future", "2027-01-01T10:00:00", "2027-01-01T11:00:00"),
    ] {
        events::create(&pool, GUILD, &input(name, start, end), created_at)
            .await
            .unwrap();
    }
    // 終日予定の end_at は終了日の 0:00 で、実際には翌日 0:00 まで続く。
    // 下限 (9/1 10:00) の日に終わる終日予定は含め、前日に終わるものは含めない
    for (name, start, end) in [
        (
            "all-day-ends-on-cutoff-day",
            "2025-09-01T00:00:00",
            "2025-09-01T00:00:00",
        ),
        (
            "all-day-ended-before",
            "2025-08-31T00:00:00",
            "2025-08-31T00:00:00",
        ),
    ] {
        let mut all_day = input(name, start, end);
        all_day.is_all_day = true;
        events::create(&pool, GUILD, &all_day, created_at)
            .await
            .unwrap();
    }
    // 他ギルドの予定は含めない
    events::create(
        &pool,
        OTHER_GUILD,
        &input("other", "2026-09-01T10:00:00", "2026-09-01T11:00:00"),
        created_at,
    )
    .await
    .unwrap();

    let rows = events::list_for_feed(&pool, GUILD, dt("2025-09-01T10:00:00"))
        .await
        .unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    // 開始日時順
    assert_eq!(
        names,
        vec!["spanning", "all-day-ends-on-cutoff-day", "edge", "future"]
    );
}
