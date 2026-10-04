-- api / bot を停止し、バックアップを取得してから実行する。
-- 端末に保存した言語は失われ、旧版では日本語の通知に戻る。
BEGIN;
DROP TRIGGER guild_locale_changed ON guilds;
DROP FUNCTION refresh_poll_language();
ALTER TABLE push_subscriptions DROP COLUMN locale;
COMMIT;
