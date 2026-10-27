-- #362 (補充済み範囲の隙間の修正) より前のイメージへ戻すときに手動実行する。
-- api / bot を停止し、DB をバックアップしてから psql -v ON_ERROR_STOP=1 -1 で流す。
-- 詳細は docs/operations.md「マイグレーションが入った版から戻す」を参照。
--
-- このマイグレーションは補充済み範囲の記録を消しただけで、スキーマと予定の行は変えていない。
-- 旧 api の起動時に VersionMissing にならないよう、適用記録だけを戻す。
DELETE FROM _sqlx_migrations WHERE version = 20261027000000;
