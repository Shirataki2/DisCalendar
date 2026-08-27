//! DB を使う統合テスト。`#[sqlx::test]` がテストごとに一時 DB を作って `migrations/` を適用する。
//! 実行には `DATABASE_URL` (api/.env でも可) で接続できる Postgres が必要。

use chrono::NaiveDateTime;
use discalendar_api::models::{
    events::{self, EventInput},
    guilds,
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
    let found = events::find_by_id_for_update(&mut *tx, GUILD, created.id)
        .await
        .unwrap();
    assert_eq!(found.map(|e| e.name).as_deref(), Some("tx"));
    // 他ギルドの ID では見えない
    assert!(
        events::find_by_id_for_update(&mut *tx, OTHER_GUILD, created.id)
            .await
            .unwrap()
            .is_none()
    );
    tx.rollback().await.unwrap();

    // ロールバックしたので残っていない
    assert!(
        events::find_by_id_for_update(&pool, GUILD, created.id)
            .await
            .unwrap()
            .is_none()
    );
}
