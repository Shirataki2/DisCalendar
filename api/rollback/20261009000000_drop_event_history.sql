-- #165 より前のイメージへ戻すときに手動実行する。記録した予定の変更履歴はすべて失われる (予定そのものは残る)。
-- api / bot を停止し、DB をバックアップしてから psql -v ON_ERROR_STOP=1 で流す。
-- 詳細は docs/operations.md「マイグレーションが入った版から戻す」を参照。
BEGIN;
DROP TABLE event_history;
DELETE FROM _sqlx_migrations WHERE version = 20261009000000;
COMMIT;
