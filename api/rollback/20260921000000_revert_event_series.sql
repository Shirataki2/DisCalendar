-- API/Botを停止してから実行。生成済みの回と添付・共有リンクは単発として保持する。
-- ルールと例外は退避し、未生成の将来分は旧版では補充されない。
BEGIN;
CREATE TABLE event_series_rollback_backup AS TABLE event_series;
CREATE TABLE event_series_exceptions_rollback_backup AS TABLE event_series_exceptions;
DROP TABLE event_series_exceptions;
ALTER TABLE events DROP COLUMN series_id, DROP COLUMN original_start_at;
DROP TABLE event_series;
DELETE FROM _sqlx_migrations WHERE version = 20260921000000;
COMMIT;
