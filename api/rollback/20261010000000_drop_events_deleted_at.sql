-- #159 (ゴミ箱) より前のイメージへ戻すときに手動実行する。
-- api / bot を停止し、DB をバックアップしてから psql -v ON_ERROR_STOP=1 -1 で流す。
-- 詳細は docs/operations.md「マイグレーションが入った版から戻す」を参照。
--
-- 先にゴミ箱の中身 (論理削除された予定) を完全に消す。列だけ落とすと、旧 api / bot は
-- deleted_at を見ないので、消したはずの予定がカレンダーと通知に復活してしまう。
-- ゴミ箱に残っていた予定は元に戻せなくなる (必要なものは戻す前に web から復元しておく)
DELETE FROM events WHERE deleted_at IS NOT NULL;
DROP INDEX idx_events_deleted_at;
ALTER TABLE events
    DROP COLUMN deleted_at,
    DROP COLUMN deleted_by;
-- 旧 api の起動時に VersionMissing にならないよう、適用記録も戻す。
DELETE FROM _sqlx_migrations WHERE version = 20261010000000;
