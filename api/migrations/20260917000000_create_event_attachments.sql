-- 添付は予定に従って削除する。guilds は Bot 退出時に消えるため外部キーを張らない。
CREATE TABLE event_attachments (
    id TEXT PRIMARY KEY,
    event_id INTEGER NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    guild_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    size BIGINT NOT NULL CHECK (size BETWEEN 1 AND 10485760),
    content_type TEXT NOT NULL CHECK (content_type IN ('image/jpeg', 'image/png', 'image/webp', 'application/pdf')),
    temporary_key TEXT NOT NULL UNIQUE,
    object_key TEXT NOT NULL UNIQUE,
    ready BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT now() + INTERVAL '24 hours'
);
CREATE INDEX ON event_attachments (guild_id);
CREATE INDEX ON event_attachments (event_id);
CREATE INDEX ON event_attachments (expires_at) WHERE NOT ready;

-- 親行と独立させ、直接 SQL・Bot・管理画面からの削除でも回収予定を失わない。
CREATE TABLE attachment_deletions (
    object_key TEXT PRIMARY KEY,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    attempts INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX ON attachment_deletions (next_attempt_at);
CREATE FUNCTION queue_attachment_deletion() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    INSERT INTO attachment_deletions (object_key, next_attempt_at)
    VALUES (OLD.temporary_key, GREATEST(now(), OLD.created_at + INTERVAL '15 minutes')),
           (OLD.object_key, now() + INTERVAL '15 minutes')
    ON CONFLICT (object_key) DO NOTHING;
    RETURN OLD;
END;
$$;
CREATE TRIGGER attachment_deleted AFTER DELETE ON event_attachments
FOR EACH ROW EXECUTE FUNCTION queue_attachment_deletion();
