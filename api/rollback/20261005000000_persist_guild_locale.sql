-- api / bot を停止し、バックアップを取得してから実行する。
-- 参加中の guilds.locale は維持する。退出済みサーバーの保存言語は失われる。
BEGIN;
ALTER TABLE guild_config DROP COLUMN locale;
DELETE FROM _sqlx_migrations WHERE version = 20261005000000;
COMMIT;
