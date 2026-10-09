-- 予定のゴミ箱 (#159)。削除は deleted_at を入れるだけにして、一定期間は元に戻せるようにする。
-- deleted_at は created_at / updated_at と同じ JST naive、deleted_by は削除した人の Discord ユーザー ID
-- (管理コンソールや AI アシスタントからの削除でも操作した人の ID が入る)。
-- 削除から 30 日を過ぎた行は bot の定期タスク (tasks/trash_cleanup.rs) が完全に消す
ALTER TABLE events
    ADD COLUMN deleted_at TIMESTAMP,
    ADD COLUMN deleted_by TEXT;

-- ゴミ箱の一覧 (ギルドごとに新しい順) と期限切れの定期削除用。削除済みの行だけを持つ小さな部分インデックス
CREATE INDEX idx_events_deleted_at ON events (guild_id, deleted_at) WHERE deleted_at IS NOT NULL;
