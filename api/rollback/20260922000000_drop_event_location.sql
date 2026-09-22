-- このロールバックを適用すると、入力済みの場所・URLは失われる。
ALTER TABLE events DROP COLUMN location;
DELETE FROM _sqlx_migrations WHERE version = 20260922000000;
