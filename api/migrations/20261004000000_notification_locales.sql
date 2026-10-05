-- 既存端末は日本語の通知を維持する。
ALTER TABLE push_subscriptions ADD COLUMN locale TEXT NOT NULL DEFAULT 'ja';

-- サーバー言語を変更したら、既存の投票メッセージも通常の同期処理で更新する。
CREATE FUNCTION refresh_poll_language() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    UPDATE schedule_polls SET discord_revision = discord_revision + 1 WHERE guild_id = NEW.guild_id;
    RETURN NEW;
END;
$$;
CREATE TRIGGER guild_locale_changed AFTER UPDATE OF locale ON guilds
FOR EACH ROW WHEN (OLD.locale IS DISTINCT FROM NEW.locale) EXECUTE FUNCTION refresh_poll_language();
