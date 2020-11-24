-- Your SQL goes here
CREATE TABLE guilds (
    id SERIAL PRIMARY KEY NOT NULL,
    guild_id TEXT NOT NULL,
    name TEXT NOT NULL,
    avatar_url TEXT,
    locale TEXT NOT NULL DEFAULT 'ja',
    UNIQUE (guild_id)
)