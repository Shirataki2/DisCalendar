-- Add migration script here
-- 通知タスク (bot/src/tasks/notify.rs) が毎分 `start_at >= $1` で全ギルド横断の
-- 未来予定を取得するので、events.start_at にインデックスが無いとテーブル全体を
-- スキャンすることになる。既存の列・データはそのままなので旧 Bot / 旧 Web との互換性には影響しない
CREATE INDEX idx_events_start_at ON events (start_at);
