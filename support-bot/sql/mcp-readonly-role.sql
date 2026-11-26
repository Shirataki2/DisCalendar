-- サポート記録の MCP (#425) が使う、読み取り専用のロール discalendar_support_mcp を作る (何度流しても同じ結果になる)。
--
-- 1 つのロールで、サポート用 DB と本体 DB の決まったテーブルだけを SELECT できるようにする。
-- パスワードはこのファイルに書かない。流したあとに psql の \password で設定する (手順は support-bot/README.md)。
--
--   psql -U discalendar -d postgres -v support_db=discalendar_support -v main_db=discalendar -f support-bot/sql/mcp-readonly-role.sql
--
-- 接続ユーザーには CREATEROLE (または superuser) と、両方の DB で GRANT できる権限 (所有者) が要る。
-- 表を足しても自動では見えない (ALL TABLES や DEFAULT PRIVILEGES は使わない)。見せる表を増やすときはここに足して流し直す。
-- pg_restore --no-privileges で戻した DB には権限が残らないので、復元したら流し直す。

\set ON_ERROR_STOP on

SELECT NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'discalendar_support_mcp') AS create_role \gset
\if :create_role
CREATE ROLE discalendar_support_mcp NOLOGIN;
\endif
-- 属性は毎回明示的に落とす (手で付けられていても戻す)。パスワードを設定するまではログインできない
ALTER ROLE discalendar_support_mcp LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS NOINHERIT
    CONNECTION LIMIT 4;
-- 権限とは別に、セッションの既定も読み取り専用にする (MCP の接続も同じ設定で開く)
ALTER ROLE discalendar_support_mcp SET default_transaction_read_only = on;
ALTER ROLE discalendar_support_mcp SET statement_timeout = '10s';

-- サポート用 DB: チケット・発言・リマインドだけ。補完用の一時的な表 (support_message_tombstones /
-- support_pending_edits) は、まだ記録していない (削除されたかもしれない) 本文を持ち得るので見せない
\connect :"support_db"
GRANT CONNECT ON DATABASE :"support_db" TO discalendar_support_mcp;
GRANT USAGE ON SCHEMA public TO discalendar_support_mcp;
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM discalendar_support_mcp;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM discalendar_support_mcp;
GRANT SELECT ON support_threads, support_messages, support_reminders TO discalendar_support_mcp;

-- 本体 DB: ギルドに Bot が参加しているか・閲覧制限・通知先を調べるための 3 表だけ。
-- 予定 (events)・ログインのセッション (Better Auth の表)・Webhook の URL などは見せない
\connect :"main_db"
GRANT CONNECT ON DATABASE :"main_db" TO discalendar_support_mcp;
GRANT USAGE ON SCHEMA public TO discalendar_support_mcp;
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM discalendar_support_mcp;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM discalendar_support_mcp;
GRANT SELECT ON guilds, guild_config, event_settings TO discalendar_support_mcp;
