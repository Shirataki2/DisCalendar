-- #172 より前へ戻すときは api / bot を停止し、DB バックアップ後に実行する。
-- まとめ投稿の設定・投稿済み日付は失われる。api / bot は同じ版へ戻す。
DROP TABLE guild_digest_settings;
DELETE FROM _sqlx_migrations WHERE version = 20260928000000;
