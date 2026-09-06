-- #170 より前の版へ戻す場合だけ、api / bot を停止し、DB のバックアップ後に実行する。
-- psql -v ON_ERROR_STOP=1 -1 < このファイル
-- 編集ロールの設定は失われる。再適用時は空配列に戻る。
ALTER TABLE guild_config DROP COLUMN editor_role_ids;
DELETE FROM _sqlx_migrations WHERE version = 20260906160000;
