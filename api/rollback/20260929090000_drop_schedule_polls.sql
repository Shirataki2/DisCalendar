-- 日程調整と回答はすべて失われる。確定して作成済みの予定は残る。
BEGIN;
DROP TABLE schedule_poll_votes, schedule_poll_options, schedule_polls;
DELETE FROM _sqlx_migrations WHERE version = 20260929090000;
COMMIT;
