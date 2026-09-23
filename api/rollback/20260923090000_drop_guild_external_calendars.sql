-- #173 Phase(b) が入った版より前へ戻すときに、API 停止・DB バックアップ後に実行する。
-- 登録 URL と取得状態は失われる。再適用後の再登録が必要。
DROP TABLE guild_external_calendars;
DELETE FROM _sqlx_migrations WHERE version = 20260923090000;
