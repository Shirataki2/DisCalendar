-- #93 より前の版へ戻す場合だけ、api / bot を停止し、DB のバックアップ後に実行する。
-- psql -v ON_ERROR_STOP=1 -1 < このファイル
-- 保存したメンション先は失われる。再適用時は空配列に戻る。
ALTER TABLE events DROP COLUMN notification_mentions;
DELETE FROM _sqlx_migrations WHERE version = 20260912090000;
