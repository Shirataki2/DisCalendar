-- 回答による更新は編集用versionと分け、投票中でも候補の確定を妨げない。
ALTER TABLE schedule_polls ADD COLUMN discord_revision BIGINT NOT NULL DEFAULT 1;
CREATE TABLE schedule_poll_posts (
    poll_id INTEGER PRIMARY KEY REFERENCES schedule_polls(id) ON DELETE CASCADE,
    channel_id TEXT NOT NULL,
    message_id TEXT,
    synced_revision BIGINT NOT NULL DEFAULT 0,
    closed BOOLEAN NOT NULL DEFAULT FALSE,
    next_attempt_at TIMESTAMP NOT NULL DEFAULT 'epoch'
);
CREATE FUNCTION bump_poll_discord_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.discord_revision := OLD.discord_revision + 1;
    RETURN NEW;
END;
$$;
CREATE TRIGGER schedule_poll_changed BEFORE UPDATE ON schedule_polls
    FOR EACH ROW EXECUTE FUNCTION bump_poll_discord_revision();
CREATE FUNCTION touch_voted_poll() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE schedule_polls SET discord_revision = discord_revision + 1
    WHERE id = (SELECT poll_id FROM schedule_poll_options WHERE id = NEW.option_id);
    RETURN NEW;
END;
$$;
CREATE TRIGGER schedule_poll_voted AFTER INSERT OR UPDATE ON schedule_poll_votes
    FOR EACH ROW EXECUTE FUNCTION touch_voted_poll();
