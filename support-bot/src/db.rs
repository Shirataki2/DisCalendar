//! サポート用 DB の読み書き。Discord への送信は持たず、イベント処理と定期タスクから呼ぶ。

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgPool;

use crate::ticket::Status;

/// スレッド (チケット) の作成・更新に使う値
#[derive(Debug, Clone)]
pub struct ThreadInput {
    pub thread_id: i64,
    pub parent_channel_id: i64,
    pub author_id: Option<i64>,
    pub title: String,
    pub tags: Vec<String>,
    pub archived: bool,
}

/// スレッドを登録する。既にあればスレッド名・タグ・アーカイブ状態だけを更新する
/// (状態は発言とタグの変化から別に決めるので、ここでは触らない)。
/// 保存期間を過ぎて本文を消したまま解決済みのスレッドには、スレッド名を書き戻さない
/// (再投稿で再オープンしたら書き戻し、次に解決してから保存期間が経ったら改めて消す)
pub async fn upsert_thread(pool: &PgPool, thread: &ThreadInput) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO support_threads (thread_id, parent_channel_id, author_id, title, tags, archived)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (thread_id) DO UPDATE SET
            title = CASE
                -- Discord で削除済みのスレッド (補完が削除前に取得した一覧から届いた) には書き戻さない
                WHEN support_threads.deleted_at IS NOT NULL THEN NULL
                WHEN support_threads.status = 'resolved'
                    AND support_threads.purged_at >= support_threads.resolved_at THEN NULL
                ELSE EXCLUDED.title
            END,
            tags = EXCLUDED.tags,
            archived = EXCLUDED.archived,
            author_id = COALESCE(support_threads.author_id, EXCLUDED.author_id),
            updated_at = now()
        "#,
        thread.thread_id,
        thread.parent_channel_id,
        thread.author_id,
        thread.title,
        &thread.tags,
        thread.archived,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn thread_exists(pool: &PgPool, thread_id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM support_threads WHERE thread_id = $1 AND deleted_at IS NULL) AS "exists!""#,
        thread_id
    )
    .fetch_one(pool)
    .await
}

/// 記録済みのスレッドの親フォーラム。記録対象から外したフォーラムかどうかの判定に使う
pub async fn thread_parent(pool: &PgPool, thread_id: i64) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT parent_channel_id FROM support_threads WHERE thread_id = $1 AND deleted_at IS NULL",
        thread_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn thread_status(pool: &PgPool, thread_id: i64) -> Result<Option<Status>, sqlx::Error> {
    let status = sqlx::query_scalar!(
        "SELECT status FROM support_threads WHERE thread_id = $1 AND deleted_at IS NULL",
        thread_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(status.as_deref().map(Status::from_db))
}

/// 状態が変わる前と後。タグの同期 (変わったときだけ Discord を編集する) に使う
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub before: Status,
    pub after: Status,
}

impl Transition {
    pub fn changed(&self) -> bool {
        self.before != self.after
    }

    /// 解決済みへ / から変わったか (「解決済み」タグの付け外しが要るか)
    pub fn resolved_changed(&self) -> bool {
        self.changed() && (self.before == Status::Resolved || self.after == Status::Resolved)
    }
}

/// 添付ファイルの記録 (本文と同じく保存期間で消す)
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AttachmentRecord {
    pub filename: String,
    pub url: String,
    pub size: u32,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MessageInput {
    pub message_id: i64,
    pub channel_id: i64,
    /// フォーラムの投稿なら thread_id (= channel_id)、テキストチャンネルなら None
    pub thread_id: Option<i64>,
    pub author_id: i64,
    pub author_is_staff: bool,
    pub content: String,
    pub attachments: Vec<AttachmentRecord>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
}

/// メッセージを記録する。既に記録済み (取りこぼしの補完と Gateway の両方から届いた) なら何もせず false を返す。
/// スレッドの発言なら、最終発言の時刻と状態も同じトランザクションで進める
pub async fn record_message(
    pool: &PgPool,
    message: &MessageInput,
) -> Result<Option<Transition>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    // スレッドの発言なら、INSERT より先に親のスレッド行をロックする。
    // - スレッドの削除 (mark_thread_deleted) と排他にし、削除済みのスレッドには保存しない
    //   (補完が取得してから保存するまでの間にスレッドごと消された場合)
    // - INSERT の外部キー検査で KEY SHARE を取ったあとに FOR UPDATE へ上げると、同じスレッドの発言を
    //   並行して記録したときにデッドロックし得るので、先に FOR NO KEY UPDATE を取っておく
    if let Some(thread_id) = message.thread_id {
        let alive = sqlx::query_scalar!(
            r#"SELECT deleted_at IS NULL AS "alive!" FROM support_threads WHERE thread_id = $1 FOR NO KEY UPDATE"#,
            thread_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        if alive != Some(true) {
            return Ok(None);
        }
    }
    let attachments =
        serde_json::to_value(&message.attachments).expect("attachments are always serializable");
    let inserted = sqlx::query!(
        r#"
        INSERT INTO support_messages
            (message_id, channel_id, thread_id, author_id, author_is_staff, content, attachments, created_at, edited_at)
        SELECT $1::bigint, $2::bigint, $3::bigint, $4::bigint, $5::boolean, $6::text, $7::jsonb, $8::timestamptz, $9::timestamptz
        -- 取得してから保存するまでの間に削除されたメッセージは保存しない (support_message_tombstones)
        WHERE NOT EXISTS (SELECT 1 FROM support_message_tombstones WHERE message_id = $1)
        ON CONFLICT (message_id) DO NOTHING
        "#,
        message.message_id,
        message.channel_id,
        message.thread_id,
        message.author_id,
        message.author_is_staff,
        message.content,
        attachments,
        message.created_at,
        message.edited_at,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if inserted {
        // 取得してから保存するまでの間に届いた編集を当て直す (support_pending_edits)
        sqlx::query!(
            r#"
            UPDATE support_messages m SET
                content = COALESCE(p.content, m.content),
                attachments = COALESCE(p.attachments, m.attachments),
                edited_at = p.edited_at
            FROM support_pending_edits p
            WHERE m.message_id = $1 AND p.message_id = $1
                AND p.edited_at > COALESCE(m.edited_at, '-infinity')
            "#,
            message.message_id,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM support_pending_edits WHERE message_id = $1",
            message.message_id
        )
        .execute(&mut *tx)
        .await?;
    }

    let transition = match (inserted, message.thread_id) {
        (true, Some(thread_id)) => {
            apply_message(
                &mut tx,
                thread_id,
                message.author_is_staff,
                message.created_at,
            )
            .await?
        }
        _ => None,
    };
    tx.commit().await?;
    Ok(transition)
}

/// 発言をスレッドの状態に反映する。
/// - 利用者の発言 → open (解決済みでも再オープン)
/// - スタッフの発言 → waiting_user (解決済みならそのまま)
///
/// 補完で古い発言が後から届いても状態を巻き戻さないよう、既知の最終発言より新しいときだけ状態を変える
async fn apply_message(
    tx: &mut sqlx::PgConnection,
    thread_id: i64,
    is_staff: bool,
    at: DateTime<Utc>,
) -> Result<Option<Transition>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        WITH before AS (
            SELECT status FROM support_threads WHERE thread_id = $1 FOR NO KEY UPDATE
        ), latest AS (
            SELECT $3 >= COALESCE(GREATEST(last_user_msg_at, last_staff_msg_at), '-infinity') AS newest
            FROM support_threads WHERE thread_id = $1
        )
        UPDATE support_threads SET
            last_user_msg_at = CASE WHEN $2 THEN last_user_msg_at ELSE GREATEST(last_user_msg_at, $3) END,
            last_staff_msg_at = CASE WHEN $2 THEN GREATEST(last_staff_msg_at, $3) ELSE last_staff_msg_at END,
            status = CASE
                WHEN NOT latest.newest THEN support_threads.status
                WHEN $2 AND support_threads.status = 'resolved' THEN 'resolved'
                WHEN $2 THEN 'waiting_user'
                ELSE 'open'
            END,
            resolved_at = CASE
                WHEN latest.newest AND NOT $2 THEN NULL
                ELSE support_threads.resolved_at
            END,
            updated_at = now()
        FROM before, latest
        WHERE support_threads.thread_id = $1
        RETURNING before.status AS before, support_threads.status AS after
        "#,
        thread_id,
        is_staff,
        at,
    )
    .fetch_optional(&mut *tx)
    .await?;
    Ok(row.map(|row| Transition {
        before: Status::from_db(&row.before),
        after: Status::from_db(&row.after),
    }))
}

/// 編集を反映する (履歴は持たず上書き)。削除済み・保存期間切れのものは書き戻さない
pub async fn update_message(
    pool: &PgPool,
    message_id: i64,
    content: Option<&str>,
    attachments: Option<&[AttachmentRecord]>,
    edited_at: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let attachments =
        attachments.map(|a| serde_json::to_value(a).expect("attachments are always serializable"));
    let mut tx = pool.begin().await?;
    let updated = sqlx::query!(
        r#"
        UPDATE support_messages SET
            content = COALESCE($2, content),
            attachments = COALESCE($3, attachments),
            edited_at = $4
        WHERE message_id = $1 AND deleted_at IS NULL AND purged_at IS NULL
            -- 補完が新しい編集を保存したあとに、古い編集イベントが届いても巻き戻さない
            AND (edited_at IS NULL OR edited_at <= $4)
        "#,
        message_id,
        content,
        attachments,
        edited_at,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        // まだ記録していないメッセージなら、補完がこのあと古い本文を保存するかもしれないので覚えておく
        // (record_message が保存時に当て直す)。記録済みで削除・保存期間切れのものは何もしない
        sqlx::query!(
            r#"
            INSERT INTO support_pending_edits (message_id, content, attachments, edited_at)
            SELECT $1, $2, $3, $4
            WHERE NOT EXISTS (SELECT 1 FROM support_messages WHERE message_id = $1)
            ON CONFLICT (message_id) DO UPDATE SET
                content = COALESCE(EXCLUDED.content, support_pending_edits.content),
                attachments = COALESCE(EXCLUDED.attachments, support_pending_edits.attachments),
                edited_at = GREATEST(EXCLUDED.edited_at, support_pending_edits.edited_at),
                received_at = now()
            "#,
            message_id,
            content,
            attachments,
            edited_at,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(updated > 0)
}

/// 削除されたメッセージは、本文と添付をその場で消して削除時刻だけを残す。
/// まだ記録していないメッセージ (補完が取得済みで保存前のものなど) も、あとから保存しないよう ID を覚えておく
pub async fn mark_messages_deleted(pool: &PgPool, message_ids: &[i64]) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        INSERT INTO support_message_tombstones (message_id)
        SELECT unnest($1::bigint[])
        ON CONFLICT (message_id) DO NOTHING
        "#,
        message_ids,
    )
    .execute(&mut *tx)
    .await?;
    let result = sqlx::query!(
        r#"
        UPDATE support_messages SET content = NULL, attachments = NULL, deleted_at = now()
        WHERE message_id = ANY($1) AND deleted_at IS NULL
        "#,
        message_ids,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(result.rows_affected())
}

/// 1 日より前の削除の記録と、記録前に届いた編集を消す (補完の取得と保存の間の食い違いを防ぐためだけに持っている)
pub async fn prune_tombstones(pool: &PgPool, now: DateTime<Utc>) -> Result<u64, sqlx::Error> {
    let cutoff = now - chrono::Duration::days(1);
    let tombstones = sqlx::query!(
        "DELETE FROM support_message_tombstones WHERE deleted_at < $1",
        cutoff,
    )
    .execute(pool)
    .await?
    .rows_affected();
    let edits = sqlx::query!(
        "DELETE FROM support_pending_edits WHERE received_at < $1",
        cutoff,
    )
    .execute(pool)
    .await?
    .rows_affected();
    Ok(tombstones + edits)
}

/// Discord でスレッドごと消されたら、スレッド名と配下の本文を消し、未送信のリマインドも取り消す
pub async fn mark_thread_deleted(pool: &PgPool, thread_id: i64) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query!(
        r#"
        UPDATE support_threads SET title = NULL, deleted_at = now(), updated_at = now()
        WHERE thread_id = $1 AND deleted_at IS NULL
        "#,
        thread_id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    sqlx::query!(
        r#"
        UPDATE support_messages SET content = NULL, attachments = NULL, deleted_at = COALESCE(deleted_at, now())
        WHERE thread_id = $1
        "#,
        thread_id
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "DELETE FROM support_reminders WHERE thread_id = $1 AND sent_at IS NULL",
        thread_id
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(updated > 0)
}

/// `/ticket status` や「解決済み」タグの付け外しで状態を変える。
/// `None` を渡すと「解決済みを外す」扱いで、最後の発言がスタッフなら waiting_user、そうでなければ open に戻す
pub async fn set_status(
    pool: &PgPool,
    thread_id: i64,
    status: Option<Status>,
) -> Result<Option<Transition>, sqlx::Error> {
    let status = status.map(Status::as_db);
    let row = sqlx::query!(
        r#"
        WITH before AS (
            SELECT status FROM support_threads WHERE thread_id = $1 AND deleted_at IS NULL FOR NO KEY UPDATE
        )
        UPDATE support_threads SET
            status = COALESCE($2, CASE
                WHEN last_staff_msg_at IS NOT NULL
                    AND last_staff_msg_at >= COALESCE(last_user_msg_at, '-infinity') THEN 'waiting_user'
                ELSE 'open'
            END),
            resolved_at = CASE
                WHEN COALESCE($2, '') = 'resolved' THEN COALESCE(resolved_at, now())
            END,
            updated_at = now()
        FROM before
        WHERE support_threads.thread_id = $1
        RETURNING before.status AS before, support_threads.status AS after
        "#,
        thread_id,
        status,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| Transition {
        before: Status::from_db(&row.before),
        after: Status::from_db(&row.after),
    }))
}

/// チャンネル (スレッド) ごとの記録済みの最後のメッセージ ID。起動時、Gateway に繋ぐ前に読んでおき、
/// 取りこぼしの補完の起点にする (繋いだあとに届いた新着で起点が進むと、停止中の発言を飛ばしてしまう)
pub async fn last_message_ids(
    pool: &PgPool,
) -> Result<std::collections::HashMap<i64, i64>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT channel_id, max(message_id) AS "last!" FROM support_messages GROUP BY channel_id"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| (r.channel_id, r.last)).collect())
}

/// 未解決チケットの一覧 (未返信の通知と日次まとめに使う)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenThread {
    pub thread_id: i64,
    pub title: Option<String>,
    pub status: Status,
    pub last_user_msg_at: Option<DateTime<Utc>>,
    pub last_staff_msg_at: Option<DateTime<Utc>>,
}

/// 利用者の最終発言から `threshold` 以上スタッフが返信していないチケット。
/// 同じ発言に対しては一度だけ通知する (reminded_at が最終発言より後なら除く)
pub async fn unreplied_threads(
    pool: &PgPool,
    now: DateTime<Utc>,
    threshold: chrono::Duration,
) -> Result<Vec<OpenThread>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT thread_id, title, status, last_user_msg_at, last_staff_msg_at
        FROM support_threads
        WHERE status = 'open' AND deleted_at IS NULL
            AND last_user_msg_at <= $1
            AND (reminded_at IS NULL OR reminded_at < last_user_msg_at)
        ORDER BY last_user_msg_at
        "#,
        now - threshold,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| OpenThread {
            thread_id: row.thread_id,
            title: row.title,
            status: Status::from_db(&row.status),
            last_user_msg_at: row.last_user_msg_at,
            last_staff_msg_at: row.last_staff_msg_at,
        })
        .collect())
}

pub async fn mark_reminded(
    pool: &PgPool,
    thread_ids: &[i64],
    at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE support_threads SET reminded_at = $2 WHERE thread_id = ANY($1)",
        thread_ids,
        at,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// 未解決 (open / waiting_user) のチケット。古い順
pub async fn unresolved_threads(pool: &PgPool) -> Result<Vec<OpenThread>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT thread_id, title, status, last_user_msg_at, last_staff_msg_at
        FROM support_threads
        WHERE status <> 'resolved' AND deleted_at IS NULL
        ORDER BY created_at, thread_id
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| OpenThread {
            thread_id: row.thread_id,
            title: row.title,
            status: Status::from_db(&row.status),
            last_user_msg_at: row.last_user_msg_at,
            last_staff_msg_at: row.last_staff_msg_at,
        })
        .collect())
}

pub async fn insert_reminder(
    pool: &PgPool,
    thread_id: i64,
    remind_at: DateTime<Utc>,
    created_by: i64,
    note: Option<&str>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"
        INSERT INTO support_reminders (thread_id, remind_at, created_by, note)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
        thread_id,
        remind_at,
        created_by,
        note,
    )
    .fetch_one(pool)
    .await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueReminder {
    pub id: i64,
    pub thread_id: i64,
    pub created_by: i64,
    pub note: Option<String>,
    pub title: Option<String>,
}

pub async fn due_reminders(
    pool: &PgPool,
    now: DateTime<Utc>,
) -> Result<Vec<DueReminder>, sqlx::Error> {
    sqlx::query_as!(
        DueReminder,
        r#"
        SELECT r.id, r.thread_id, r.created_by, r.note, t.title
        FROM support_reminders r
        JOIN support_threads t USING (thread_id)
        WHERE r.sent_at IS NULL AND r.remind_at <= $1 AND t.deleted_at IS NULL
        ORDER BY r.remind_at, r.id
        "#,
        now,
    )
    .fetch_all(pool)
    .await
}

pub async fn mark_reminder_sent(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE support_reminders SET sent_at = now() WHERE id = $1",
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// 1 日 1 回のタスクを、その日 (JST) にまだ実行していなければ true
pub async fn daily_task_pending(
    pool: &PgPool,
    name: &str,
    today: NaiveDate,
) -> Result<bool, sqlx::Error> {
    let last = sqlx::query_scalar!(
        "SELECT last_run_on FROM support_task_runs WHERE name = $1",
        name
    )
    .fetch_optional(pool)
    .await?;
    Ok(last.is_none_or(|last| last < today))
}

pub async fn mark_daily_task_done(
    pool: &PgPool,
    name: &str,
    today: NaiveDate,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO support_task_runs (name, last_run_on) VALUES ($1, $2)
        ON CONFLICT (name) DO UPDATE SET last_run_on = GREATEST(support_task_runs.last_run_on, EXCLUDED.last_run_on)
        "#,
        name,
        today,
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// 保存期間を過ぎた本文を消した件数
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Purged {
    pub threads: u64,
    pub messages: u64,
}

/// 保存期間を過ぎた本文・添付・スレッド名を消す (行とメタ情報は残す)。
/// - フォーラムの投稿: 解決してから `retention` 経ったスレッド
/// - テキストチャンネルの発言: 投稿から `retention` 経ったもの
pub async fn purge_expired(
    pool: &PgPool,
    now: DateTime<Utc>,
    retention: chrono::Duration,
) -> Result<Purged, sqlx::Error> {
    let cutoff = now - retention;
    let mut tx = pool.begin().await?;
    let threads = sqlx::query_scalar!(
        r#"
        UPDATE support_threads SET title = NULL, purged_at = $2, updated_at = $2
        WHERE status = 'resolved' AND resolved_at <= $1
            -- 一度消したあとに再投稿・再解決されたスレッドも、新しい解決時刻から数えて消す
            AND (purged_at IS NULL OR purged_at < resolved_at)
        RETURNING thread_id
        "#,
        cutoff,
        now,
    )
    .fetch_all(&mut *tx)
    .await?;
    // 発言はスレッドの purged_at と切り離し、解決から保存期間が経ったスレッドの「まだ消していない発言」をすべて消す。
    // 消し終えた解決済みのスレッドに (解決済みのまま) スタッフが返信した分も、ここで拾う
    let thread_messages = sqlx::query!(
        r#"
        UPDATE support_messages m SET content = NULL, attachments = NULL, purged_at = $2
        FROM support_threads t
        WHERE m.thread_id = t.thread_id AND m.purged_at IS NULL
            AND t.status = 'resolved' AND t.resolved_at <= $1
        "#,
        cutoff,
        now,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let channel_messages = sqlx::query!(
        r#"
        UPDATE support_messages SET content = NULL, attachments = NULL, purged_at = $2
        WHERE thread_id IS NULL AND created_at <= $1 AND purged_at IS NULL
        "#,
        cutoff,
        now,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    tx.commit().await?;
    Ok(Purged {
        threads: threads.len() as u64,
        messages: thread_messages + channel_messages,
    })
}
