-- 作成・更新の操作者と更新日時。移行前の予定は不明 (NULL) のまま残す。
ALTER TABLE events
    ADD COLUMN created_by TEXT,
    ADD COLUMN updated_by TEXT,
    ADD COLUMN updated_at TIMESTAMP;
