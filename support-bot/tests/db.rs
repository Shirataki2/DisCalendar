//! サポート用 DB の読み書き (状態遷移・未返信の判定・リマインド・日次まとめ・保存期間) のテスト。
//! `#[sqlx::test]` がテストごとに一時データベースを作り、support-bot のマイグレーションを当てる。

use chrono::{DateTime, Duration, NaiveDate, TimeZone as _, Utc};
use discalendar_support_bot::{
    db::{self, AttachmentRecord, MessageInput, Purged, ThreadInput, Transition},
    ticket::Status,
};
use sqlx::PgPool;

const THREAD: i64 = 1_000;
const FORUM: i64 = 10;
const USER: i64 = 100;
const STAFF: i64 = 200;

fn at(hours: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap() + Duration::hours(hours)
}

async fn thread(pool: &PgPool, thread_id: i64) {
    db::upsert_thread(
        pool,
        &ThreadInput {
            thread_id,
            parent_channel_id: FORUM,
            author_id: Some(USER),
            title: format!("投稿 {thread_id}"),
            tags: vec!["不具合".into()],
            archived: false,
        },
    )
    .await
    .unwrap();
}

fn message(
    id: i64,
    thread_id: Option<i64>,
    staff: bool,
    created_at: DateTime<Utc>,
) -> MessageInput {
    MessageInput {
        message_id: id,
        channel_id: thread_id.unwrap_or(FORUM + 1),
        thread_id,
        author_id: if staff { STAFF } else { USER },
        author_is_staff: staff,
        content: format!("本文 {id}"),
        attachments: vec![AttachmentRecord {
            filename: "a.png".into(),
            url: "https://cdn.example/a.png".into(),
            size: 10,
            content_type: Some("image/png".into()),
        }],
        created_at,
        edited_at: None,
    }
}

async fn status(pool: &PgPool, thread_id: i64) -> Status {
    db::thread_status(pool, thread_id).await.unwrap().unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn messages_move_ticket_status(pool: PgPool) {
    thread(&pool, THREAD).await;
    assert_eq!(status(&pool, THREAD).await, Status::Open);

    let t = db::record_message(&pool, &message(1, Some(THREAD), false, at(0)))
        .await
        .unwrap();
    assert_eq!(
        t,
        Some(Transition {
            before: Status::Open,
            after: Status::Open
        })
    );

    let t = db::record_message(&pool, &message(2, Some(THREAD), true, at(1)))
        .await
        .unwrap();
    assert_eq!(
        t,
        Some(Transition {
            before: Status::Open,
            after: Status::WaitingUser
        })
    );

    // 同じメッセージがもう一度届いても (補完と Gateway の重複) 状態は動かない
    assert_eq!(
        db::record_message(&pool, &message(2, Some(THREAD), true, at(1)))
            .await
            .unwrap(),
        None
    );

    // 補完で古い利用者の発言が後から届いても、状態は巻き戻らない
    db::record_message(
        &pool,
        &message(3, Some(THREAD), false, at(0) + Duration::minutes(30)),
    )
    .await
    .unwrap();
    assert_eq!(status(&pool, THREAD).await, Status::WaitingUser);

    // 解決済みにしたあとスタッフが書いても解決済みのまま、利用者が書くと再オープン
    let t = db::set_status(&pool, THREAD, Some(Status::Resolved))
        .await
        .unwrap()
        .unwrap();
    assert!(t.resolved_changed());
    db::record_message(&pool, &message(4, Some(THREAD), true, at(2)))
        .await
        .unwrap();
    assert_eq!(status(&pool, THREAD).await, Status::Resolved);
    let t = db::record_message(&pool, &message(5, Some(THREAD), false, at(3)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        t,
        Transition {
            before: Status::Resolved,
            after: Status::Open
        }
    );
    assert!(t.resolved_changed());
}

#[sqlx::test(migrations = "./migrations")]
async fn removing_resolved_returns_to_status_by_last_message(pool: PgPool) {
    thread(&pool, THREAD).await;
    db::record_message(&pool, &message(1, Some(THREAD), false, at(0)))
        .await
        .unwrap();
    db::record_message(&pool, &message(2, Some(THREAD), true, at(1)))
        .await
        .unwrap();
    db::set_status(&pool, THREAD, Some(Status::Resolved))
        .await
        .unwrap();

    let t = db::set_status(&pool, THREAD, None).await.unwrap().unwrap();
    assert_eq!(
        t,
        Transition {
            before: Status::Resolved,
            after: Status::WaitingUser
        }
    );

    // 存在しないスレッドは None
    assert_eq!(
        db::set_status(&pool, 9_999, Some(Status::Open))
            .await
            .unwrap(),
        None
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn channel_messages_do_not_touch_threads(pool: PgPool) {
    let t = db::record_message(&pool, &message(1, None, false, at(0)))
        .await
        .unwrap();
    assert_eq!(t, None);
}

#[sqlx::test(migrations = "./migrations")]
async fn last_message_ids_by_channel(pool: PgPool) {
    thread(&pool, THREAD).await;
    for (id, thread_id) in [(1, None), (5, None), (3, Some(THREAD)), (2, Some(THREAD))] {
        db::record_message(&pool, &message(id, thread_id, false, at(id)))
            .await
            .unwrap();
    }
    let last = db::last_message_ids(&pool).await.unwrap();
    assert_eq!(last.get(&(FORUM + 1)), Some(&5));
    assert_eq!(last.get(&THREAD), Some(&3));
    assert_eq!(last.get(&12_345), None);
}

#[sqlx::test(migrations = "./migrations")]
async fn thread_parent_is_known_for_recorded_threads(pool: PgPool) {
    thread(&pool, THREAD).await;
    assert_eq!(db::thread_parent(&pool, THREAD).await.unwrap(), Some(FORUM));
    assert_eq!(db::thread_parent(&pool, 9_999).await.unwrap(), None);
    db::mark_thread_deleted(&pool, THREAD).await.unwrap();
    assert_eq!(db::thread_parent(&pool, THREAD).await.unwrap(), None);
}

#[sqlx::test(migrations = "./migrations")]
async fn edits_and_deletes(pool: PgPool) {
    thread(&pool, THREAD).await;
    db::record_message(&pool, &message(1, Some(THREAD), false, at(0)))
        .await
        .unwrap();

    assert!(
        db::update_message(&pool, 1, Some("直した"), None, at(1))
            .await
            .unwrap()
    );
    let row = sqlx::query!(
        "SELECT content, attachments, edited_at FROM support_messages WHERE message_id = 1"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.content.as_deref(), Some("直した"));
    assert!(row.attachments.is_some(), "添付は届いていないので残す");
    assert_eq!(row.edited_at, Some(at(1)));

    assert_eq!(db::mark_messages_deleted(&pool, &[1, 2]).await.unwrap(), 1);
    let row = sqlx::query!(
        "SELECT content, attachments, deleted_at FROM support_messages WHERE message_id = 1"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((row.content, row.attachments), (None, None));
    assert!(row.deleted_at.is_some());
    // 消したあとに編集イベントが届いても書き戻さない
    assert!(
        !db::update_message(&pool, 1, Some("復活"), None, at(2))
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn deleting_thread_clears_contents_and_reminders(pool: PgPool) {
    thread(&pool, THREAD).await;
    db::record_message(&pool, &message(1, Some(THREAD), false, at(0)))
        .await
        .unwrap();
    db::insert_reminder(&pool, THREAD, at(1), STAFF, None)
        .await
        .unwrap();

    assert!(db::mark_thread_deleted(&pool, THREAD).await.unwrap());
    assert!(!db::mark_thread_deleted(&pool, THREAD).await.unwrap());
    assert!(!db::thread_exists(&pool, THREAD).await.unwrap());
    let content = sqlx::query_scalar!("SELECT content FROM support_messages WHERE message_id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(content, None);
    assert!(db::due_reminders(&pool, at(2)).await.unwrap().is_empty());
    assert!(db::unresolved_threads(&pool).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn unreplied_threads_are_notified_once_per_user_message(pool: PgPool) {
    let threshold = Duration::hours(24);
    thread(&pool, THREAD).await;
    db::record_message(&pool, &message(1, Some(THREAD), false, at(0)))
        .await
        .unwrap();

    // 24 時間ちょうどで対象になる
    assert!(
        db::unreplied_threads(&pool, at(23), threshold)
            .await
            .unwrap()
            .is_empty()
    );
    let found = db::unreplied_threads(&pool, at(24), threshold)
        .await
        .unwrap();
    assert_eq!(
        found.iter().map(|t| t.thread_id).collect::<Vec<_>>(),
        [THREAD]
    );

    // 通知済みなら同じ発言では再通知しない
    db::mark_reminded(&pool, &[THREAD], at(24)).await.unwrap();
    assert!(
        db::unreplied_threads(&pool, at(30), threshold)
            .await
            .unwrap()
            .is_empty()
    );

    // 利用者が書き足したら、そこから 24 時間でもう一度
    db::record_message(&pool, &message(2, Some(THREAD), false, at(31)))
        .await
        .unwrap();
    assert_eq!(
        db::unreplied_threads(&pool, at(55), threshold)
            .await
            .unwrap()
            .len(),
        1
    );

    // スタッフが返信すれば対象外
    db::record_message(&pool, &message(3, Some(THREAD), true, at(56)))
        .await
        .unwrap();
    assert!(
        db::unreplied_threads(&pool, at(100), threshold)
            .await
            .unwrap()
            .is_empty()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn unresolved_threads_lists_open_and_waiting(pool: PgPool) {
    for id in [1, 2, 3] {
        thread(&pool, id).await;
    }
    db::record_message(&pool, &message(20, Some(2), true, at(0)))
        .await
        .unwrap();
    db::set_status(&pool, 3, Some(Status::Resolved))
        .await
        .unwrap();

    let threads = db::unresolved_threads(&pool).await.unwrap();
    let got: Vec<_> = threads.iter().map(|t| (t.thread_id, t.status)).collect();
    assert_eq!(got, [(1, Status::Open), (2, Status::WaitingUser)]);
}

#[sqlx::test(migrations = "./migrations")]
async fn reminders_become_due(pool: PgPool) {
    thread(&pool, THREAD).await;
    let id = db::insert_reminder(&pool, THREAD, at(5), STAFF, Some("確認する"))
        .await
        .unwrap();

    assert!(db::due_reminders(&pool, at(4)).await.unwrap().is_empty());
    let due = db::due_reminders(&pool, at(5)).await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(
        (due[0].id, due[0].created_by, due[0].note.as_deref()),
        (id, STAFF, Some("確認する"))
    );

    db::mark_reminder_sent(&pool, id).await.unwrap();
    assert!(db::due_reminders(&pool, at(10)).await.unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations")]
async fn daily_task_runs_once_per_day(pool: PgPool) {
    let day = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    assert!(
        db::daily_task_pending(&pool, "daily_summary", day)
            .await
            .unwrap()
    );
    db::mark_daily_task_done(&pool, "daily_summary", day)
        .await
        .unwrap();
    assert!(
        !db::daily_task_pending(&pool, "daily_summary", day)
            .await
            .unwrap()
    );
    let next = day.succ_opt().unwrap();
    assert!(
        db::daily_task_pending(&pool, "daily_summary", next)
            .await
            .unwrap()
    );
    // 日付が戻っても実行済みの日は巻き戻さない
    db::mark_daily_task_done(&pool, "daily_summary", next)
        .await
        .unwrap();
    db::mark_daily_task_done(&pool, "daily_summary", day)
        .await
        .unwrap();
    assert!(
        !db::daily_task_pending(&pool, "daily_summary", next)
            .await
            .unwrap()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn purges_expired_contents(pool: PgPool) {
    let retention = Duration::days(180);
    // 解決済み (古い) / 解決済み (新しい) / 未解決 (古い)
    for id in [1, 2, 3] {
        thread(&pool, id).await;
        db::record_message(&pool, &message(id * 10, Some(id), false, at(0)))
            .await
            .unwrap();
    }
    db::set_status(&pool, 1, Some(Status::Resolved))
        .await
        .unwrap();
    db::set_status(&pool, 2, Some(Status::Resolved))
        .await
        .unwrap();
    let now = Utc::now();
    sqlx::query!(
        "UPDATE support_threads SET resolved_at = $1 WHERE thread_id = 1",
        now - Duration::days(181)
    )
    .execute(&pool)
    .await
    .unwrap();
    // テキストチャンネルの発言 (古い / 新しい)
    db::record_message(&pool, &message(100, None, false, now - Duration::days(181)))
        .await
        .unwrap();
    db::record_message(&pool, &message(101, None, false, now - Duration::days(1)))
        .await
        .unwrap();

    let purged = db::purge_expired(&pool, now, retention).await.unwrap();
    assert_eq!(
        purged,
        Purged {
            threads: 1,
            messages: 2
        }
    );

    let rows = sqlx::query!("SELECT message_id, content IS NULL AS \"purged!\" FROM support_messages ORDER BY message_id")
        .fetch_all(&pool)
        .await
        .unwrap();
    let got: Vec<_> = rows.iter().map(|r| (r.message_id, r.purged)).collect();
    assert_eq!(
        got,
        [
            (10, true),
            (20, false),
            (30, false),
            (100, true),
            (101, false)
        ]
    );
    let title = sqlx::query_scalar!("SELECT title FROM support_threads WHERE thread_id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title, None);

    // 2 回目は何も消さない。消したスレッドにスレッド名を書き戻さない
    assert_eq!(
        db::purge_expired(&pool, now, retention).await.unwrap(),
        Purged::default()
    );
    thread(&pool, 1).await;
    let title = sqlx::query_scalar!("SELECT title FROM support_threads WHERE thread_id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title, None);
}

#[sqlx::test(migrations = "./migrations")]
async fn purges_again_after_reopen_and_resolve(pool: PgPool) {
    let retention = Duration::days(180);
    let now = Utc::now();
    thread(&pool, THREAD).await;
    db::record_message(
        &pool,
        &message(1, Some(THREAD), false, now - Duration::days(401)),
    )
    .await
    .unwrap();
    db::set_status(&pool, THREAD, Some(Status::Resolved))
        .await
        .unwrap();
    let old = now - Duration::days(400);
    sqlx::query!(
        "UPDATE support_threads SET resolved_at = $1 WHERE thread_id = $2",
        old,
        THREAD
    )
    .execute(&pool)
    .await
    .unwrap();
    let first = now - Duration::days(200);
    assert_eq!(
        db::purge_expired(&pool, first, retention).await.unwrap(),
        Purged {
            threads: 1,
            messages: 1
        }
    );

    // 消したあとに利用者が再投稿 → 再オープンでスレッド名を書き戻し、本文も記録する
    db::record_message(
        &pool,
        &message(2, Some(THREAD), false, now - Duration::days(199)),
    )
    .await
    .unwrap();
    assert_eq!(status(&pool, THREAD).await, Status::Open);
    thread(&pool, THREAD).await;
    let title = sqlx::query_scalar!(
        "SELECT title FROM support_threads WHERE thread_id = $1",
        THREAD
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(title.is_some());

    // 再び解決して 180 日経ったら、新しい本文とスレッド名も消す
    db::set_status(&pool, THREAD, Some(Status::Resolved))
        .await
        .unwrap();
    let resolved_again = now - Duration::days(181);
    sqlx::query!(
        "UPDATE support_threads SET resolved_at = $1 WHERE thread_id = $2",
        resolved_again,
        THREAD
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        db::purge_expired(&pool, now, retention).await.unwrap(),
        Purged {
            threads: 1,
            messages: 1
        }
    );
    let remaining = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!" FROM support_messages WHERE content IS NOT NULL"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(remaining, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn purges_staff_replies_to_already_purged_threads(pool: PgPool) {
    let retention = Duration::days(180);
    let now = Utc::now();
    thread(&pool, THREAD).await;
    db::record_message(
        &pool,
        &message(1, Some(THREAD), false, now - Duration::days(400)),
    )
    .await
    .unwrap();
    db::set_status(&pool, THREAD, Some(Status::Resolved))
        .await
        .unwrap();
    let resolved_at = now - Duration::days(390);
    sqlx::query!(
        "UPDATE support_threads SET resolved_at = $1 WHERE thread_id = $2",
        resolved_at,
        THREAD
    )
    .execute(&pool)
    .await
    .unwrap();
    db::purge_expired(&pool, now - Duration::days(200), retention)
        .await
        .unwrap();

    // 消したあと、解決済みのままスタッフが返信した (状態も resolved_at も変わらない)
    db::record_message(
        &pool,
        &message(2, Some(THREAD), true, now - Duration::days(10)),
    )
    .await
    .unwrap();
    assert_eq!(status(&pool, THREAD).await, Status::Resolved);

    let purged = db::purge_expired(&pool, now, retention).await.unwrap();
    assert_eq!(
        purged,
        Purged {
            threads: 0,
            messages: 1
        }
    );
    let content = sqlx::query_scalar!("SELECT content FROM support_messages WHERE message_id = 2")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(content, None);
}

#[sqlx::test(migrations = "./migrations")]
async fn orders_messages_within_the_same_second(pool: PgPool) {
    thread(&pool, THREAD).await;
    let staff_at = at(0) + Duration::milliseconds(100);
    db::record_message(&pool, &message(1, Some(THREAD), true, staff_at))
        .await
        .unwrap();
    db::record_message(
        &pool,
        &message(2, Some(THREAD), false, staff_at + Duration::milliseconds(1)),
    )
    .await
    .unwrap();
    assert_eq!(status(&pool, THREAD).await, Status::Open);
    // 解決して「解決済み」を外すと、最後の発言 (利用者) に合わせて対応待ちに戻る
    db::set_status(&pool, THREAD, Some(Status::Resolved))
        .await
        .unwrap();
    db::set_status(&pool, THREAD, None).await.unwrap();
    assert_eq!(status(&pool, THREAD).await, Status::Open);
}

#[sqlx::test(migrations = "./migrations")]
async fn deleted_before_recording_is_not_saved(pool: PgPool) {
    thread(&pool, THREAD).await;
    // 補完が取得したあと、保存する前に Discord で削除された
    assert_eq!(db::mark_messages_deleted(&pool, &[7]).await.unwrap(), 0);
    let t = db::record_message(&pool, &message(7, Some(THREAD), false, at(0)))
        .await
        .unwrap();
    assert_eq!(t, None);
    let rows = sqlx::query_scalar!(r#"SELECT count(*) AS "count!" FROM support_messages"#)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);

    // 1 日経った削除の記録は消える
    assert_eq!(
        db::prune_tombstones(&pool, Utc::now() + Duration::days(2))
            .await
            .unwrap(),
        1
    );
}
