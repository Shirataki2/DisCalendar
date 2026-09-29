-- Discord投票メッセージの追跡情報を失う。候補・回答と確定済み予定は保持する。
-- 第2段階のBotを停止してから実行する。既存のDiscordメッセージは手動で削除する。
BEGIN;
DROP TRIGGER schedule_poll_voted ON schedule_poll_votes;
DROP FUNCTION touch_voted_poll();
DROP TRIGGER schedule_poll_changed ON schedule_polls;
DROP FUNCTION bump_poll_discord_revision();
DROP TABLE schedule_poll_posts;
ALTER TABLE schedule_polls DROP COLUMN discord_revision;
DELETE FROM _sqlx_migrations WHERE version = 20260929100000;
COMMIT;
