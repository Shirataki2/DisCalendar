-- API/Bot停止後、他の繰り返し戻しSQLより先に実行する。
BEGIN;
DROP VIEW event_creations;
-- 戻した版が初回を作成として数えられるようにする。
UPDATE events e SET generated_from_series=false FROM event_series s
WHERE e.series_id=s.id AND e.original_start_at=s.start_at AND s.is_creation;
ALTER TABLE event_series DROP COLUMN is_creation;
DELETE FROM _sqlx_migrations WHERE version=20260921000002;
COMMIT;
