-- 自動実行されない。api と bot を停止し、DB をバックアップしてから
-- psql -v ON_ERROR_STOP=1 -1 で実行し、旧版へ戻す。
-- 登録 URL・シークレット・未配信の予定・配信ログは失われ、再登録が必要。
DROP TABLE guild_webhook_deliveries;
DROP TABLE guild_webhook_outbox;
DROP TABLE guild_webhooks;
DELETE FROM _sqlx_migrations WHERE version = 20260913000000;
