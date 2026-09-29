BEGIN;
DROP INDEX schedule_poll_votes_user;
DELETE FROM _sqlx_migrations WHERE version = 20260929092000;
COMMIT;
