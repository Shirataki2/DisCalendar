//! 予定のゴミ箱 (#159)。削除は論理削除で、一定期間は元に戻せる。
//!
//! 行が残るので、予定を読む経路のどこかで `deleted_at IS NULL` を忘れると「消したはずの予定」が出てくる。
//! 読む経路を 1 か所ずつ確かめ、経路が増えたときに漏れに気付けるようにする
//! (bot 側の経路は `bot/tests/events.rs` で同じように確かめる)

use chrono::NaiveDateTime;
use discalendar_api::{
    mcp,
    models::{
        admin_guilds, admin_ops, admin_stats, attachments, event_links,
        events::{self, EventInput},
        shares,
    },
    recurring::{self, DeleteOptions},
    recurring_events as store,
};
use sqlx::PgPool;

const GUILD: &str = "111";

fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}

fn input(name: &str) -> EventInput {
    EventInput {
        recurrence: None,
        scope: Default::default(),
        expected_series_version: None,
        name: name.into(),
        description: Some("説明".into()),
        location: None,
        notifications: serde_json::from_value(serde_json::json!([{"num": 30, "unit": "minutes"}]))
            .unwrap(),
        notification_mentions: None,
        color: "#5865F2".into(),
        is_all_day: false,
        start_at: dt("2026-10-05T19:00:00"),
        end_at: dt("2026-10-05T20:00:00"),
        discord_scheduled_event: None,
    }
}

async fn guild(pool: &PgPool) {
    sqlx::query("INSERT INTO guilds (guild_id, name, locale) VALUES ($1, 'サーバー', 'ja')")
        .bind(GUILD)
        .execute(pool)
        .await
        .unwrap();
}

async fn create(pool: &PgPool, name: &str) -> i32 {
    let body = input(name);
    events::create(pool, GUILD, &body, dt("2026-10-01T00:00:00"), "333")
        .await
        .unwrap()
        .id
}

async fn trash(pool: &PgPool, id: i32, at: &str) -> bool {
    let mut conn = pool.acquire().await.unwrap();
    events::soft_delete(&mut conn, GUILD, id, "444", dt(at))
        .await
        .unwrap()
}

/// 期間内の予定名 (カレンダーの一覧と同じクエリ)
async fn names(pool: &PgPool) -> Vec<String> {
    events::list_between(
        pool,
        GUILD,
        dt("2026-10-01T00:00:00"),
        dt("2026-11-01T00:00:00"),
    )
    .await
    .unwrap()
    .into_iter()
    .map(|e| e.name)
    .collect()
}

#[sqlx::test(migrations = "./migrations")]
async fn trashed_events_do_not_appear_anywhere(pool: PgPool) {
    guild(&pool).await;
    sqlx::query("INSERT INTO event_settings (guild_id, channel_id) VALUES ($1, '999')")
        .bind(GUILD)
        .execute(&pool)
        .await
        .unwrap();
    let kept = create(&pool, "残す予定").await;
    let gone = create(&pool, "消す予定").await;
    let link = shares::issue(&pool, GUILD, gone).await.unwrap().unwrap();
    event_links::insert(&pool, GUILD, gone, "9001", dt("2026-10-01T00:00:00"))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO push_outbox (event_id, fire_at, start_at) SELECT id, start_at, start_at FROM events",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(trash(&pool, gone, "2026-10-02T09:00:00").await);
    // 2 回目は何もしない (削除した人・日時を上書きしない)
    assert!(!trash(&pool, gone, "2026-10-03T09:00:00").await);

    let (from, to) = (dt("2026-10-01T00:00:00"), dt("2026-11-01T00:00:00"));
    // カレンダーの一覧
    assert_eq!(names(&pool).await, ["残す予定"]);
    // 横断カレンダー
    let joined = events::list_between_guilds(&pool, &[GUILD.to_owned()], from, to)
        .await
        .unwrap();
    assert_eq!(joined.iter().map(|e| e.id).collect::<Vec<_>>(), [kept]);
    // iCal フィード
    let feed = events::list_for_feed(&pool, GUILD, from).await.unwrap();
    assert_eq!(feed.iter().map(|e| e.id).collect::<Vec<_>>(), [kept]);
    // AI アシスタント (MCP) の一覧
    let listed = mcp::list_rows(&pool, GUILD, from, to, None, 100)
        .await
        .unwrap();
    assert_eq!(listed.iter().map(|e| e.id).collect::<Vec<_>>(), [kept]);
    // 1 件の取得 (更新・共有リンクの発行・履歴・添付ファイルの起点)
    assert!(
        events::find_by_id(&pool, GUILD, gone)
            .await
            .unwrap()
            .is_none()
    );
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        events::find_by_id_for_update(&mut conn, GUILD, gone)
            .await
            .unwrap()
            .is_none()
    );
    // 更新はできない
    let renamed = input("書き換え");
    assert!(
        events::update(&pool, GUILD, gone, &renamed, "333", from)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        events::update_if_unlinked(&pool, GUILD, gone, &renamed, "333", from)
            .await
            .unwrap()
            .is_none()
    );
    // 共有ページ: 削除中は見えず、新しく発行もできない
    assert!(shares::find(&pool, &link.token).await.unwrap().is_none());
    assert!(shares::get(&pool, GUILD, gone).await.unwrap().is_none());
    assert!(shares::issue(&pool, GUILD, gone).await.unwrap().is_none());
    // 添付ファイルの操作の起点
    assert!(
        attachments::lock_event(&mut conn, GUILD, gone)
            .await
            .is_err()
    );
    // 管理コンソールの件数と通知の見込み
    let admin = admin_guilds::find(&pool, GUILD).await.unwrap().unwrap();
    assert_eq!(admin.event_count, 1);
    let listed = admin_guilds::list(&pool, "", 1).await.unwrap();
    assert_eq!(listed[0].event_count, 1);
    // 残した 1 件ぶん (開始時刻 + 30 分前) だけ
    assert_eq!(
        admin_stats::notifications_between(&pool, from, to)
            .await
            .unwrap(),
        2
    );
    // Discord イベントとの対応付けと送信待ちのプッシュ通知は、ゴミ箱に入れた時点で消える
    assert!(
        event_links::get(&pool, GUILD, gone)
            .await
            .unwrap()
            .is_none()
    );
    let queued: Vec<i32> = sqlx::query_scalar("SELECT event_id FROM push_outbox")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(queued, [kept]);
}

#[sqlx::test(migrations = "./migrations")]
async fn restore_brings_the_event_back_as_it_was(pool: PgPool) {
    guild(&pool).await;
    let id = create(&pool, "定例").await;
    let link = shares::issue(&pool, GUILD, id).await.unwrap().unwrap();

    // ゴミ箱に無い予定は復元の対象外
    assert!(events::restore(&pool, GUILD, id).await.unwrap().is_none());
    assert!(trash(&pool, id, "2026-10-02T09:00:00").await);
    // 他ギルドからは復元できない
    assert!(events::restore(&pool, "222", id).await.unwrap().is_none());

    let restored = events::restore(&pool, GUILD, id).await.unwrap().unwrap();
    // 日時・通知設定はそのまま (通知は bot が行から読むので、復元すればまた発火する)
    assert_eq!(restored.start_at, dt("2026-10-05T19:00:00"));
    assert_eq!(restored.end_at, dt("2026-10-05T20:00:00"));
    assert_eq!(
        restored.notifications,
        serde_json::json!([{"num": 30, "unit": "minutes"}])
    );
    assert_eq!(restored.description.as_deref(), Some("説明"));
    assert_eq!(names(&pool).await, ["定例"]);
    // 共有リンクは同じ URL で復活する
    assert_eq!(
        shares::find(&pool, &link.token)
            .await
            .unwrap()
            .unwrap()
            .name,
        "定例"
    );
    // 削除の記録は残らない
    let (deleted_at, deleted_by): (Option<NaiveDateTime>, Option<String>) =
        sqlx::query_as("SELECT deleted_at, deleted_by FROM events WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((deleted_at, deleted_by), (None, None));
}

#[sqlx::test(migrations = "./migrations")]
async fn trash_lists_newest_first_and_purge_only_takes_trashed_events(pool: PgPool) {
    guild(&pool).await;
    let first = create(&pool, "先に消した").await;
    let second = create(&pool, "後で消した").await;
    let alive = create(&pool, "消していない").await;
    assert!(trash(&pool, first, "2026-10-02T09:00:00").await);
    assert!(trash(&pool, second, "2026-10-03T09:00:00").await);

    let listed = events::list_trash(&pool, GUILD).await.unwrap();
    assert_eq!(
        listed.iter().map(|e| e.id).collect::<Vec<_>>(),
        [second, first]
    );
    assert_eq!(listed[0].deleted_by.as_deref(), Some("444"));
    assert_eq!(listed[0].deleted_at, dt("2026-10-03T09:00:00"));
    assert_eq!(listed[0].expires_at, dt("2026-11-02T09:00:00"));
    // 他ギルドのゴミ箱には出ない
    assert!(events::list_trash(&pool, "222").await.unwrap().is_empty());
    // 削除した人の名前を引けるよう、操作者の確認に含める
    assert_eq!(
        events::author_ids(&pool, GUILD, &["444".to_owned()])
            .await
            .unwrap(),
        ["444"]
    );

    // 完全削除はゴミ箱の予定だけ。他ギルドからも消せない
    assert!(!events::purge(&pool, GUILD, alive).await.unwrap());
    assert!(!events::purge(&pool, "222", first).await.unwrap());
    assert!(events::purge(&pool, GUILD, first).await.unwrap());
    assert!(
        events::restore(&pool, GUILD, first)
            .await
            .unwrap()
            .is_none()
    );
    let listed = events::list_trash(&pool, GUILD).await.unwrap();
    assert_eq!(listed.iter().map(|e| e.id).collect::<Vec<_>>(), [second]);
    assert_eq!(names(&pool).await, ["消していない"]);
}

#[sqlx::test(migrations = "./migrations")]
async fn trashed_occurrence_leaves_the_series_and_is_not_regenerated(pool: PgPool) {
    guild(&pool).await;
    let mut body = input("毎週の定例");
    body.recurrence = Some(
        serde_json::from_value(serde_json::json!({
            "frequency": "weekly", "weekdays": [0], "end": {"type": "never"}
        }))
        .unwrap(),
    );
    let mut tx = pool.begin().await.unwrap();
    let row = events::create(&mut *tx, GUILD, &body, body.start_at, "333")
        .await
        .unwrap();
    recurring::attach_created(&mut tx, GUILD, row.id, &body, "333")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let (from, to) = (dt("2026-10-01T00:00:00"), dt("2026-11-01T00:00:00"));
    store::ensure_range(&pool, GUILD, from, to).await.unwrap();
    let before = events::list_between(&pool, GUILD, from, to).await.unwrap();
    assert_eq!(before.len(), 4);
    let target = before[1].id;

    // 「この回のみ」の削除: 開催枠を中止として記録してからゴミ箱へ
    let mut tx = pool.begin().await.unwrap();
    assert!(
        !recurring::before_delete(&mut tx, GUILD, target, &DeleteOptions::default())
            .await
            .unwrap()
    );
    assert!(
        events::soft_delete(&mut tx, GUILD, target, "444", dt("2026-10-02T09:00:00"))
            .await
            .unwrap()
    );
    tx.commit().await.unwrap();

    // 補充し直しても、その回は作り直されない
    sqlx::query("UPDATE event_series SET generated_from = NULL, generated_until = NULL")
        .execute(&pool)
        .await
        .unwrap();
    store::ensure_range(&pool, GUILD, from, to).await.unwrap();
    let after = events::list_between(&pool, GUILD, from, to).await.unwrap();
    assert_eq!(after.len(), 3);
    assert!(after.iter().all(|e| e.id != target));

    // 復元すると、繰り返しから外れた単発の予定として元の日時に戻る (同じ枠が二重にならない)
    let restored = events::restore(&pool, GUILD, target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.start_at, before[1].start_at);
    store::ensure_range(&pool, GUILD, from, to).await.unwrap();
    let after = events::list_between(&pool, GUILD, from, to).await.unwrap();
    assert_eq!(after.len(), 4);
    let mut conn = pool.acquire().await.unwrap();
    assert!(
        store::info(&mut conn, GUILD, target)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store::info(&mut conn, GUILD, after[0].id)
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn deleting_all_events_of_a_guild_also_empties_the_trash(pool: PgPool) {
    guild(&pool).await;
    sqlx::query("INSERT INTO guild_webhooks (guild_id, url, kind, secret, created_by) VALUES ($1, 'http://127.0.0.1', 'json', 'test-only', '333')")
        .bind(GUILD)
        .execute(&pool)
        .await
        .unwrap();
    let alive = create(&pool, "残っていた予定").await;
    let gone = create(&pool, "ゴミ箱の予定").await;
    assert!(trash(&pool, gone, "2026-10-02T09:00:00").await);

    let mut tx = pool.begin().await.unwrap();
    let (snapshot, deleted) = admin_ops::delete_guild_events(&mut tx, GUILD, "555")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(deleted, 2);
    assert_eq!(snapshot.len(), 2);
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
    // ゴミ箱の予定は入れた時点で削除を通知済みなので、Webhook は残っていた予定の分だけ
    let queued: Vec<(Option<i32>, serde_json::Value)> =
        sqlx::query_as("SELECT event_id, payload FROM guild_webhook_outbox")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].0, Some(alive));
    // ゴミ箱用の列は Webhook の本文に出さない
    assert!(queued[0].1.get("deleted_at").is_none());
    assert!(queued[0].1.get("deleted_by").is_none());
}

#[sqlx::test(migrations = "./migrations")]
async fn rollback_purges_the_trash_before_dropping_the_columns(pool: PgPool) {
    guild(&pool).await;
    let alive = create(&pool, "残っていた予定").await;
    let gone = create(&pool, "ゴミ箱の予定").await;
    assert!(trash(&pool, gone, "2026-10-02T09:00:00").await);

    sqlx::raw_sql(include_str!(
        "../rollback/20261010000000_drop_events_deleted_at.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();

    // 列だけ落とすと、deleted_at を見ない旧 api / bot で消したはずの予定が復活してしまう
    let remaining: Vec<i32> = sqlx::query_scalar("SELECT id FROM events")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, [alive]);
    let columns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM information_schema.columns WHERE table_name = 'events' AND column_name IN ('deleted_at', 'deleted_by')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(columns, 0);
    let applied: i64 =
        sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE version = 20261010000000")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(applied, 0);
}
