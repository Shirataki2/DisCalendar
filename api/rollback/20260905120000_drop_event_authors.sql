-- #156 より前のイメージへ戻すときに手動実行する。記録した作成者・更新者は失われる。
-- api / bot を停止し、DB をバックアップしてから psql -v ON_ERROR_STOP=1 -1 で流す。
-- 詳細は README「マイグレーションが入った版から戻す」を参照。
ALTER TABLE events
    DROP COLUMN created_by,
    DROP COLUMN updated_by,
    DROP COLUMN updated_at;
DELETE FROM _sqlx_migrations WHERE version = 20260905120000;
