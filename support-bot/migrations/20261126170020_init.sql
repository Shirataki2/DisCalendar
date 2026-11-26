-- サポート用 Bot (#423) の初期スキーマ。
-- 本体 (api) とは別のデータベース (既定 discalendar_support) に置く。
-- 同じデータベースに置くと _sqlx_migrations を api と取り合い、どちらかが起動できなくなる。
-- Discord の Snowflake は 2^63 未満なので BIGINT で持つ (外に出すときは文字列にする)。

-- フォーラムの投稿 (スレッド) 1 件 = チケット 1 件
CREATE TABLE support_threads (
    thread_id BIGINT PRIMARY KEY,
    -- 投稿先のフォーラム
    parent_channel_id BIGINT NOT NULL,
    -- 起票者 (スレッドの owner)
    author_id BIGINT,
    -- スレッド名。保存期間を過ぎたら NULL にする
    title TEXT,
    -- open = スタッフの返信待ち / waiting_user = 利用者の返信待ち / resolved = 解決済み
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'waiting_user', 'resolved')),
    -- 付いているフォーラムタグの名前 (不具合 / 質問 / 要望 / 解決済み など)
    tags TEXT[] NOT NULL DEFAULT '{}',
    archived BOOLEAN NOT NULL DEFAULT FALSE,
    last_user_msg_at TIMESTAMPTZ,
    last_staff_msg_at TIMESTAMPTZ,
    -- 未返信の通知を最後に送った時刻。利用者の新しい発言より前なら、もう一度通知してよい
    reminded_at TIMESTAMPTZ,
    resolved_at TIMESTAMPTZ,
    -- 起票した GitHub Issue の番号 (Phase 3 の「Issue 化」で使う)
    github_issue INTEGER,
    -- 保存期間を過ぎて本文を消した時刻
    purged_at TIMESTAMPTZ,
    -- Discord 側でスレッドが消された時刻 (本文はその場で消す)
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT support_threads_resolved_at CHECK ((status = 'resolved') = (resolved_at IS NOT NULL))
);

CREATE INDEX support_threads_unresolved ON support_threads (last_user_msg_at)
    WHERE status <> 'resolved' AND deleted_at IS NULL;
CREATE INDEX support_threads_resolved_at ON support_threads (resolved_at)
    WHERE status = 'resolved';

-- 記録したメッセージ。thread_id が NULL のものはテキストチャンネルへの投稿
CREATE TABLE support_messages (
    message_id BIGINT PRIMARY KEY,
    channel_id BIGINT NOT NULL,
    thread_id BIGINT REFERENCES support_threads (thread_id) ON DELETE CASCADE,
    author_id BIGINT NOT NULL,
    author_is_staff BOOLEAN NOT NULL,
    -- 本文と添付 ([{filename, url, size, content_type}])。削除・保存期間切れで NULL にする
    content TEXT,
    attachments JSONB,
    created_at TIMESTAMPTZ NOT NULL,
    edited_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ,
    purged_at TIMESTAMPTZ
);

CREATE INDEX support_messages_thread ON support_messages (thread_id, created_at);
CREATE INDEX support_messages_channel ON support_messages (channel_id, message_id);
CREATE INDEX support_messages_unpurged_channel ON support_messages (created_at)
    WHERE thread_id IS NULL AND purged_at IS NULL;

-- 削除されたメッセージの ID。起動時の補完が HTTP で取得してから保存するまでの間に Discord で削除されると、
-- 削除イベントは記録前の行には効かず、取得済みの本文がそのあと保存されてしまうので、削除を覚えておいて保存を止める。
-- 補完の取得と保存の間の短い食い違いを防ぐためだけのものなので、1 日で消す
CREATE TABLE support_message_tombstones (
    message_id BIGINT PRIMARY KEY,
    deleted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- まだ記録していないメッセージに届いた編集。補完が HTTP で取得してから保存するまでの間に編集されると、
-- 編集イベントは記録前の行には効かず、取得済みの古い本文がそのあと保存されてしまうので、保存時に当て直す。
-- 削除の記録と同じく短い食い違いを防ぐためだけのものなので、1 日で消す
CREATE TABLE support_pending_edits (
    message_id BIGINT PRIMARY KEY,
    content TEXT,
    attachments JSONB,
    edited_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- /remind で設定した手動のリマインド
CREATE TABLE support_reminders (
    id BIGSERIAL PRIMARY KEY,
    thread_id BIGINT NOT NULL REFERENCES support_threads (thread_id) ON DELETE CASCADE,
    remind_at TIMESTAMPTZ NOT NULL,
    created_by BIGINT NOT NULL,
    note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    sent_at TIMESTAMPTZ
);

CREATE INDEX support_reminders_due ON support_reminders (remind_at) WHERE sent_at IS NULL;

-- 1 日 1 回の定期タスク (日次まとめ) を最後に実行した日 (JST)。再起動で二重に投稿しないために持つ
CREATE TABLE support_task_runs (
    name TEXT PRIMARY KEY,
    last_run_on DATE NOT NULL
);
