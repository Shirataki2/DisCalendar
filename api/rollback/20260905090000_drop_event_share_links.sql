-- #154 を含む版から旧版へ戻すときだけ手動で実行する。
-- api を停止し、DB のバックアップを取ってから psql -v ON_ERROR_STOP=1 -1 で流す。
-- 発行済みの共有 URL はすべて失効し、再適用しても復元されない。
DROP TABLE event_share_links;
-- 旧 api の起動時に VersionMissing にならないよう、適用記録も戻す。
DELETE FROM _sqlx_migrations WHERE version = 20260905090000;
