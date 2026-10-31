//! `events` テーブルの読み書き (`/create` と `/list` が使う)。
//! web (api) が保存した予定を Bot が読め、Bot が保存した予定を web が読める形式であることを確認する

use chrono::NaiveDateTime;
use discalendar_bot::models::{
    events::{self, NewEvent, SearchScope},
    notifications::{Notification, NotificationUnit},
};
use sqlx::PgPool;

const GUILD: &str = "111111111111111111";
const OTHER_GUILD: &str = "222222222222222222";

fn dt(s: &str) -> NaiveDateTime {
    s.parse().unwrap()
}

fn new_event<'a>(guild_id: &'a str, name: &'a str, start: &str, end: &str) -> NewEvent<'a> {
    NewEvent {
        guild_id,
        name,
        description: None,
        location: None,
        notifications: &[],
        color: "#2196F3",
        is_all_day: false,
        start_at: dt(start),
        end_at: dt(end),
        created_at: dt("2026-08-01T00:00:00"),
        created_by: "333",
    }
}

#[sqlx::test(migrations = "../api/migrations")]
async fn create_stores_notifications_in_legacy_format(pool: PgPool) {
    let notifications = [
        Notification::new(30, NotificationUnit::Minutes),
        Notification::new(1, NotificationUnit::Days),
    ];
    let event = events::create(
        &pool,
        &NewEvent {
            description: Some("説明"),
            notifications: &notifications,
            ..new_event(GUILD, "定例", "2026-08-23T10:00:00", "2026-08-23T11:00:00")
        },
    )
    .await
    .unwrap();

    assert_eq!(event.created_by.as_deref(), Some("333"));
    assert_eq!(event.updated_by, None);
    assert_eq!(event.updated_at, None);
    assert_eq!(event.guild_id, GUILD);
    assert_eq!(event.name, "定例");
    assert_eq!(event.description.as_deref(), Some("説明"));
    // DB には api / web と同じ JSONB で入る
    assert_eq!(
        event.notifications,
        serde_json::json!([
            { "num": 30, "unit": "minutes" },
            { "num": 1, "unit": "days" },
        ])
    );
    assert_eq!(event.notifications(), notifications);
    assert_eq!(event.color, "#2196F3");
    assert!(!event.is_all_day);
    assert_eq!(event.start_at, dt("2026-08-23T10:00:00"));
    assert_eq!(event.end_at, dt("2026-08-23T11:00:00"));
    assert_eq!(event.created_at, dt("2026-08-01T00:00:00"));
}

#[sqlx::test(migrations = "../api/migrations")]
async fn lists_are_scoped_to_guild_and_split_by_now(pool: PgPool) {
    let past = events::create(
        &pool,
        &new_event(GUILD, "過去", "2026-08-01T10:00:00", "2026-08-01T11:00:00"),
    )
    .await
    .unwrap();
    let future = events::create(
        &pool,
        &new_event(GUILD, "未来", "2026-08-30T10:00:00", "2026-08-30T11:00:00"),
    )
    .await
    .unwrap();
    // ちょうど now に始まる予定は過去にも未来にも出る (旧 Bot と同じ <= / >=)
    let now = events::create(
        &pool,
        &new_event(GUILD, "今", "2026-08-15T12:00:00", "2026-08-15T13:00:00"),
    )
    .await
    .unwrap();
    let other_now = events::create(
        &pool,
        &new_event(
            OTHER_GUILD,
            "他",
            "2026-08-15T12:00:00",
            "2026-08-15T13:00:00",
        ),
    )
    .await
    .unwrap();

    let other_future = events::create(
        &pool,
        &new_event(
            OTHER_GUILD,
            "他の未来",
            "2026-08-30T10:00:00",
            "2026-08-30T11:00:00",
        ),
    )
    .await
    .unwrap();

    let at = dt("2026-08-15T12:00:00");
    assert_eq!(
        events::list_all(&pool, GUILD).await.unwrap(),
        vec![past.clone(), now.clone(), future.clone()]
    );
    assert_eq!(
        events::list_past(&pool, GUILD, at).await.unwrap(),
        vec![past, now.clone()]
    );
    assert_eq!(
        events::list_future(&pool, GUILD, at).await.unwrap(),
        vec![now.clone(), future.clone()]
    );
    assert_eq!(
        events::list_all(&pool, "333333333333333333").await.unwrap(),
        vec![]
    );
    // 通知タスクはギルドを横断して未来の予定を集める (start_at, id 順)
    assert_eq!(
        events::list_all_future(&pool, at).await.unwrap(),
        vec![now, other_now, future, other_future]
    );
}

#[sqlx::test(migrations = "../api/migrations")]
async fn reads_events_saved_by_web(pool: PgPool) {
    // web (api) が保存する形: 終日予定は開始日 0:00 〜 終了日 0:00
    sqlx::query!(
        r#"
        INSERT INTO events (guild_id, name, description, notifications, color, is_all_day, start_at, end_at)
        VALUES ($1, 'web の予定', NULL, $2, '#F44336', TRUE, '2026-08-23 00:00:00', '2026-08-24 00:00:00')
        "#,
        GUILD,
        serde_json::json!([{ "num": 1, "unit": "weeks" }, "broken"])
    )
    .execute(&pool)
    .await
    .unwrap();

    let events = events::list_all(&pool, GUILD).await.unwrap();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert!(event.is_all_day);
    assert_eq!(event.start_at, dt("2026-08-23T00:00:00"));
    assert_eq!(event.end_at, dt("2026-08-24T00:00:00"));
    // 解釈できない要素は無視して読む
    assert_eq!(
        event.notifications(),
        vec![Notification::new(1, NotificationUnit::Weeks)]
    );
}

#[sqlx::test(migrations = "../api/migrations")]
async fn period_and_next_respect_boundaries_and_guild(pool: PgPool) {
    let start = dt("2026-09-11T00:00:00");
    let end = dt("2026-09-12T00:00:00");
    assert!(
        events::list_period(&pool, GUILD, start, end)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        events::list_next(&pool, GUILD, start)
            .await
            .unwrap()
            .is_empty()
    );
    for (name, from, to, all_day) in [
        (
            "終了済み",
            "2026-09-10T23:00:00",
            "2026-09-11T00:00:00",
            false,
        ),
        (
            "日付またぎ",
            "2026-09-10T23:00:00",
            "2026-09-11T01:00:00",
            false,
        ),
        (
            "終日最終日",
            "2026-09-10T00:00:00",
            "2026-09-11T00:00:00",
            true,
        ),
        (
            "終日当日",
            "2026-09-11T00:00:00",
            "2026-09-11T00:00:00",
            true,
        ),
        (
            "同時刻",
            "2026-09-11T00:00:00",
            "2026-09-11T00:00:00",
            false,
        ),
        ("翌日", "2026-09-12T00:00:00", "2026-09-12T01:00:00", false),
    ] {
        events::create(
            &pool,
            &NewEvent {
                is_all_day: all_day,
                ..new_event(GUILD, name, from, to)
            },
        )
        .await
        .unwrap();
    }
    events::create(
        &pool,
        &new_event(
            OTHER_GUILD,
            "他",
            "2026-09-11T00:00:00",
            "2026-09-11T01:00:00",
        ),
    )
    .await
    .unwrap();
    let names = |events: Vec<events::Event>| events.into_iter().map(|e| e.name).collect::<Vec<_>>();
    assert_eq!(
        names(events::list_period(&pool, GUILD, start, end).await.unwrap()),
        ["終日最終日", "日付またぎ", "終日当日", "同時刻"]
    );
    assert_eq!(
        names(events::list_next(&pool, GUILD, start).await.unwrap()),
        ["終日当日", "同時刻"]
    );
    assert_eq!(
        names(
            events::list_next(&pool, GUILD, start + chrono::Duration::seconds(1))
                .await
                .unwrap()
        ),
        ["翌日"]
    );
}

#[sqlx::test(migrations = "../api/migrations")]
async fn bot_creation_queues_webhook_in_the_same_transaction(pool: PgPool) {
    sqlx::query("INSERT INTO guilds (guild_id,name) VALUES ($1,'Webhook')")
        .bind(GUILD)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO guild_webhooks (guild_id,url,kind,secret,created_by) VALUES ($1,'https://example.com','json','test-only','333')")
        .bind(GUILD).execute(&pool).await.unwrap();
    let event = events::create(
        &pool,
        &new_event(
            GUILD,
            "Bot の予定",
            "2026-10-01T10:00:00",
            "2026-10-01T11:00:00",
        ),
    )
    .await
    .unwrap();
    let (kind, actor, payload): (String, String, serde_json::Value) =
        sqlx::query_as("SELECT kind,actor_id,payload FROM guild_webhook_outbox WHERE event_id=$1")
            .bind(event.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kind, "event.created");
    assert_eq!(actor, "333");
    assert_eq!(payload["name"], "Bot の予定");
    assert_eq!(payload["notifications"], serde_json::json!([]));
}

/// guild_id, actor, "source:action", before, after
type HistoryRow = (
    String,
    Option<String>,
    String,
    Option<serde_json::Value>,
    Option<serde_json::Value>,
);

/// `/create` で保存した予定には、api と同じ形の変更履歴 (#165) が `source = bot` で 1 行残る
#[sqlx::test(migrations = "../api/migrations")]
async fn create_records_history_from_the_bot(pool: PgPool) {
    let event = events::create(
        &pool,
        &new_event(GUILD, "定例", "2026-08-23T10:00:00", "2026-08-23T11:00:00"),
    )
    .await
    .unwrap();

    let rows: Vec<HistoryRow> = sqlx::query_as(
        "SELECT guild_id, actor_discord_user_id, source || ':' || action, before, after
             FROM event_history WHERE event_id = $1",
    )
    .bind(event.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![(
            GUILD.to_owned(),
            Some("333".to_owned()),
            "bot:create".to_owned(),
            None,
            Some(serde_json::json!({
                "name": "定例",
                "location": null,
                "color": "#2196F3",
                "is_all_day": false,
                "start_at": "2026-08-23T10:00:00",
                "end_at": "2026-08-23T11:00:00",
                "notifications": [],
                "notification_mentions": [],
                "discord_linked": false,
                "recurrence": null,
                "description_changed": false,
            })),
        )]
    );
}

/// ゴミ箱 (#159) に入っている予定は、`/list` のどの範囲にも通知の対象にも出てこない。
/// 行は残っているので、読む経路ごとに確かめる (api 側は `api/tests/trash.rs`)
#[sqlx::test(migrations = "../api/migrations")]
async fn trashed_events_are_hidden_from_lists_and_notifications(pool: PgPool) {
    let at = dt("2026-08-23T10:00:00");
    let mut ids = Vec::new();
    for (name, start, end) in [
        ("過去", "2026-08-22T10:00:00", "2026-08-22T11:00:00"),
        ("未来", "2026-08-24T10:00:00", "2026-08-24T11:00:00"),
    ] {
        let kept = events::create(&pool, &new_event(GUILD, name, start, end))
            .await
            .unwrap();
        let gone = events::create(&pool, &new_event(GUILD, "消した予定", start, end))
            .await
            .unwrap();
        ids.push((kept.id, gone.id));
    }
    let trashed: Vec<i32> = ids.iter().map(|(_, gone)| *gone).collect();
    sqlx::query("UPDATE events SET deleted_at = '2026-08-20 09:00:00', deleted_by = '444' WHERE id = ANY($1)")
        .bind(&trashed)
        .execute(&pool)
        .await
        .unwrap();

    let names = |events: Vec<events::Event>| -> Vec<String> {
        events.into_iter().map(|e| e.name).collect()
    };
    assert_eq!(
        names(events::list_all(&pool, GUILD).await.unwrap()),
        ["過去", "未来"]
    );
    assert_eq!(
        names(events::list_past(&pool, GUILD, at).await.unwrap()),
        ["過去"]
    );
    assert_eq!(
        names(events::list_future(&pool, GUILD, at).await.unwrap()),
        ["未来"]
    );
    assert_eq!(
        names(
            events::list_period(
                &pool,
                GUILD,
                dt("2026-08-22T00:00:00"),
                dt("2026-08-25T00:00:00")
            )
            .await
            .unwrap()
        ),
        ["過去", "未来"]
    );
    assert_eq!(
        names(events::list_next(&pool, GUILD, at).await.unwrap()),
        ["未来"]
    );
    // 通知タスクの対象
    assert_eq!(
        names(events::list_all_future(&pool, at).await.unwrap()),
        ["未来"]
    );

    // 元に戻せば (deleted_at を外せば) また通知の対象になる
    sqlx::query("UPDATE events SET deleted_at = NULL, deleted_by = NULL WHERE id = $1")
        .bind(ids[1].1)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        names(events::list_all_future(&pool, at).await.unwrap()),
        ["未来", "消した予定"]
    );
}

/// 次の予定 (`list_next`) は、いちばん早い予定がゴミ箱に入っていたら、その次の予定を返す
#[sqlx::test(migrations = "../api/migrations")]
async fn next_skips_a_trashed_earliest_event(pool: PgPool) {
    let at = dt("2026-08-23T10:00:00");
    let gone = events::create(
        &pool,
        &new_event(
            GUILD,
            "消した予定",
            "2026-08-24T10:00:00",
            "2026-08-24T11:00:00",
        ),
    )
    .await
    .unwrap();
    events::create(
        &pool,
        &new_event(
            GUILD,
            "次の予定",
            "2026-08-25T10:00:00",
            "2026-08-25T11:00:00",
        ),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE events SET deleted_at = '2026-08-20 09:00:00' WHERE id = $1")
        .bind(gone.id)
        .execute(&pool)
        .await
        .unwrap();
    let next = events::list_next(&pool, GUILD, at).await.unwrap();
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].name, "次の予定");
}

/// ゴミ箱に入って 30 日を過ぎた予定だけが完全に消える
#[sqlx::test(migrations = "../api/migrations")]
async fn expired_trash_is_purged(pool: PgPool) {
    let now = dt("2026-10-10T12:00:00");
    let mut ids = Vec::new();
    for name in ["期限切れ", "まだ残す", "消していない"] {
        let event = events::create(
            &pool,
            &new_event(GUILD, name, "2026-08-24T10:00:00", "2026-08-24T11:00:00"),
        )
        .await
        .unwrap();
        ids.push(event.id);
    }
    // 30 日と 1 分前 / 30 日ちょうどの 1 分手前
    for (id, deleted_at) in [
        (ids[0], "2026-09-10T11:59:00"),
        (ids[1], "2026-09-10T12:01:00"),
    ] {
        sqlx::query("UPDATE events SET deleted_at = $2 WHERE id = $1")
            .bind(id)
            .bind(dt(deleted_at))
            .execute(&pool)
            .await
            .unwrap();
    }
    assert_eq!(events::purge_expired_trash(&pool, now).await.unwrap(), 1);
    let remaining: Vec<i32> = sqlx::query_scalar("SELECT id FROM events ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, [ids[1], ids[2]]);
    assert_eq!(events::purge_expired_trash(&pool, now).await.unwrap(), 0);
}

/// `/search` (#206): タイトル・説明の部分一致。サーバーの境界を越えず、範囲の判定は `/list` と同じ
#[sqlx::test(migrations = "../api/migrations")]
async fn search_matches_title_and_description_within_the_guild(pool: PgPool) {
    let at = dt("2026-08-15T12:00:00");
    let create = |guild: &'static str, name: &'static str, description, start, end| {
        let pool = pool.clone();
        async move {
            events::create(
                &pool,
                &NewEvent {
                    description,
                    ..new_event(guild, name, start, end)
                },
            )
            .await
            .unwrap()
        }
    };
    let past = create(
        GUILD,
        "週次定例",
        None,
        "2026-08-01T10:00:00",
        "2026-08-01T11:00:00",
    )
    .await;
    let older = create(
        GUILD,
        "週次定例",
        None,
        "2026-07-01T10:00:00",
        "2026-07-01T11:00:00",
    )
    .await;
    let now = create(
        GUILD,
        "Sprint Review",
        Some("定例の振り返り"),
        "2026-08-15T12:00:00",
        "2026-08-15T13:00:00",
    )
    .await;
    let future = create(
        GUILD,
        "週次定例",
        None,
        "2026-08-30T10:00:00",
        "2026-08-30T11:00:00",
    )
    .await;
    let _unrelated = create(
        GUILD,
        "飲み会",
        Some("自由参加"),
        "2026-08-30T19:00:00",
        "2026-08-30T21:00:00",
    )
    .await;
    let _other = create(
        OTHER_GUILD,
        "週次定例",
        None,
        "2026-08-30T10:00:00",
        "2026-08-30T11:00:00",
    )
    .await;

    let search = |keyword: &'static str, scope| {
        let pool = pool.clone();
        async move {
            events::search(&pool, GUILD, keyword, scope, at, events::SEARCH_LIMIT)
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.id)
                .collect::<Vec<_>>()
        }
    };
    // 既定の未来は start_at >= now (`/list` の未来と同じ)。説明の一致も対象
    assert_eq!(
        search("定例", SearchScope::Future).await,
        [now.id, future.id]
    );
    // 過去は start_at <= now で、新しい順
    assert_eq!(
        search("定例", SearchScope::Past).await,
        [now.id, past.id, older.id]
    );
    assert_eq!(
        search("定例", SearchScope::All).await,
        [older.id, past.id, now.id, future.id]
    );
    // 英字は大文字・小文字を区別しない
    assert_eq!(search("sprint", SearchScope::All).await, [now.id]);
    assert_eq!(
        search("存在しない", SearchScope::All).await,
        Vec::<i32>::new()
    );
    // 他のサーバーからは見えない
    assert_eq!(
        events::search(&pool, OTHER_GUILD, "Sprint", SearchScope::All, at, 100)
            .await
            .unwrap(),
        vec![]
    );
    // 件数の上限
    assert_eq!(
        events::search(&pool, GUILD, "定例", SearchScope::All, at, 2)
            .await
            .unwrap()
            .len(),
        2
    );

    // 範囲の判定は `/list` の過去・未来・全てと一致する
    let ids = |events: Vec<events::Event>| -> Vec<i32> {
        events
            .into_iter()
            .filter(|e| {
                e.name.contains("定例")
                    || e.description.as_deref().is_some_and(|d| d.contains("定例"))
            })
            .map(|e| e.id)
            .collect()
    };
    assert_eq!(
        ids(events::list_future(&pool, GUILD, at).await.unwrap()),
        search("定例", SearchScope::Future).await
    );
    let mut past_ids = ids(events::list_past(&pool, GUILD, at).await.unwrap());
    past_ids.reverse();
    assert_eq!(past_ids, search("定例", SearchScope::Past).await);
    assert_eq!(
        ids(events::list_all(&pool, GUILD).await.unwrap()),
        search("定例", SearchScope::All).await
    );
}

#[sqlx::test(migrations = "../api/migrations")]
async fn search_treats_like_wildcards_literally(pool: PgPool) {
    let at = dt("2026-08-01T00:00:00");
    for name in ["100%達成", "1000達成", "a_b", "axb", r"c\d"] {
        events::create(
            &pool,
            &new_event(GUILD, name, "2026-08-23T10:00:00", "2026-08-23T11:00:00"),
        )
        .await
        .unwrap();
    }
    let names = |keyword: &'static str| {
        let pool = pool.clone();
        async move {
            events::search(&pool, GUILD, keyword, SearchScope::All, at, 100)
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(names("0%").await, ["100%達成"]);
    assert_eq!(names("a_b").await, ["a_b"]);
    assert_eq!(names(r"c\d").await, [r"c\d"]);
    assert_eq!(names("%").await, ["100%達成"]);
}

#[sqlx::test(migrations = "../api/migrations")]
async fn search_and_find_skip_trash_and_other_guilds(pool: PgPool) {
    let at = dt("2026-08-01T00:00:00");
    let kept = events::create(
        &pool,
        &new_event(GUILD, "定例", "2026-08-23T10:00:00", "2026-08-23T11:00:00"),
    )
    .await
    .unwrap();
    let gone = events::create(
        &pool,
        &new_event(GUILD, "定例", "2026-08-24T10:00:00", "2026-08-24T11:00:00"),
    )
    .await
    .unwrap();
    let other = events::create(
        &pool,
        &new_event(
            OTHER_GUILD,
            "定例",
            "2026-08-23T10:00:00",
            "2026-08-23T11:00:00",
        ),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE events SET deleted_at = '2026-08-20 09:00:00', deleted_by = '444' WHERE id = $1",
    )
    .bind(gone.id)
    .execute(&pool)
    .await
    .unwrap();

    let found = events::search(&pool, GUILD, "定例", SearchScope::All, at, 100)
        .await
        .unwrap();
    assert_eq!(found, vec![kept.clone()]);
    assert_eq!(
        events::find(&pool, GUILD, kept.id).await.unwrap(),
        Some(kept)
    );
    // 選択前にゴミ箱へ移された予定・他のサーバーの予定は取得できない
    assert_eq!(events::find(&pool, GUILD, gone.id).await.unwrap(), None);
    assert_eq!(events::find(&pool, GUILD, other.id).await.unwrap(), None);
    assert_eq!(
        events::find(&pool, OTHER_GUILD, other.id).await.unwrap(),
        Some(other)
    );
}
