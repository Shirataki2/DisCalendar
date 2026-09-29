BEGIN;
DROP INDEX schedule_polls_confirmed_event;
DELETE FROM _sqlx_migrations WHERE version = 20260929091000;
COMMIT;
