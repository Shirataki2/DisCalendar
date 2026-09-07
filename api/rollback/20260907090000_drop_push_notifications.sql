-- API / Bot を停止してから実行。端末は再登録が必要になる。
BEGIN;
DROP TABLE IF EXISTS push_deliveries, push_outbox, push_subscriptions, user_push_settings;
DELETE FROM _sqlx_migrations WHERE version = 20260907090000;
COMMIT;
