-- 予定のゴミ箱 (#159)。削除は deleted_at を入れるだけにして、一定期間は元に戻せるようにする。
-- deleted_at は created_at / updated_at と同じ JST naive、deleted_by は削除した人の Discord ユーザー ID
-- (管理コンソールや AI アシスタントからの削除でも操作した人の ID が入る)。
-- 削除から 30 日を過ぎた行は bot の定期タスク (tasks/trash_cleanup.rs) が完全に消す。
--
-- NULL 許可・既定値なしの列追加なので表の書き換えは起きない。
-- ゴミ箱用のインデックスは、書き込みを止めないよう次のマイグレーションで CONCURRENTLY に作る
ALTER TABLE events
    ADD COLUMN deleted_at TIMESTAMP,
    ADD COLUMN deleted_by TEXT;
