-- Bot が退出しても管理者の投稿言語を保持する。未保存の設定は guilds の言語を使う。
ALTER TABLE guild_config ADD COLUMN locale TEXT;
INSERT INTO guild_config (guild_id, locale)
SELECT guild_id, locale FROM guilds
ON CONFLICT (guild_id) DO UPDATE SET locale = EXCLUDED.locale;
