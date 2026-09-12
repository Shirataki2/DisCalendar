-- 事前通知が空でも開始時刻のメンション先を保持するため、予定単位で保存する。
ALTER TABLE events ADD COLUMN notification_mentions JSONB NOT NULL DEFAULT '[]'::jsonb
    CHECK (jsonb_typeof(notification_mentions) = 'array');
