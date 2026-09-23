-- 外部 ICS の読み取り専用購読。URL は限定公開トークンを含むことがある。
CREATE TABLE guild_external_calendars (
    id BIGSERIAL PRIMARY KEY,
    guild_id TEXT NOT NULL,
    url TEXT NOT NULL,
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    created_by TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    last_fetched_at TIMESTAMP,
    last_error TEXT,
    etag TEXT,
    last_modified TEXT,
    UNIQUE (guild_id, url)
);
CREATE INDEX guild_external_calendars_guild_id_idx ON guild_external_calendars (guild_id);
