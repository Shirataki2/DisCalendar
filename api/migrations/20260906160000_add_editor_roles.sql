-- 指定した Discord ロールにも予定の編集を許可する (#170)
ALTER TABLE guild_config ADD COLUMN editor_role_ids TEXT[] NOT NULL DEFAULT '{}';
