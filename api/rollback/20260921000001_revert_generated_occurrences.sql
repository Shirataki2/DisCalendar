-- API/Botを停止し、繰り返しテーブルの戻しSQLより先に実行する。
BEGIN;
ALTER TABLE events DROP COLUMN generated_from_series;
DELETE FROM _sqlx_migrations WHERE version = 20260921000001;
COMMIT;
