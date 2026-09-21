BEGIN;
ALTER TABLE event_series DROP COLUMN generated_from;
DELETE FROM _sqlx_migrations WHERE version=20260921000003;
COMMIT;
