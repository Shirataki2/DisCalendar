-- 20261128000000_create_guild_api_tokens.sql の戻し方

UPDATE event_series SET created_by = '' WHERE created_by IS NULL;
ALTER TABLE event_series ALTER COLUMN created_by SET NOT NULL;

DELETE FROM event_history WHERE source = 'api_token';
ALTER TABLE event_history DROP CONSTRAINT IF EXISTS event_history_source_check;
ALTER TABLE event_history
    ADD CONSTRAINT event_history_source_check
    CHECK (source IN ('web', 'bot', 'admin', 'mcp'));
ALTER TABLE event_history DROP COLUMN IF EXISTS actor_label;

DROP TABLE IF EXISTS event_external_ids;
DROP TABLE IF EXISTS guild_api_token_operations;
DROP TABLE IF EXISTS guild_api_token_logs;
DROP TABLE IF EXISTS guild_api_tokens;

-- 古い api の migrations/ に無いバージョンを消す (残っていると起動時に VersionMissing で落ちる)
DELETE FROM _sqlx_migrations WHERE version = 20261128000000;
