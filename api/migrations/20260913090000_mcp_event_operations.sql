-- 本文を含む再送結果は24時間。キーの墓標は再作成防止のため保持する。
CREATE TABLE mcp_event_operations (
    user_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    guild_id TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('create', 'update', 'delete')),
    event_id INTEGER NOT NULL,
    result JSONB,
    discord_event_id TEXT,
    discord_state TEXT NOT NULL CHECK (discord_state IN ('not_required', 'succeeded', 'failed', 'unknown')),
    requires_discord_create BOOLEAN NOT NULL,
    requires_bot_create BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, client_id, guild_id, key_hash)
);
CREATE INDEX mcp_event_operations_expiry ON mcp_event_operations (created_at) WHERE result IS NOT NULL;
CREATE INDEX mcp_event_operations_unresolved ON mcp_event_operations (event_id) WHERE discord_state IN ('unknown', 'failed');
