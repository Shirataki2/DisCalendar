-- #217: 同意ごとの許可は不変。再同意では別 ID を発行し、旧トークンを拡張しない。
CREATE TABLE mcp_connections (
  id text PRIMARY KEY,
  user_id text NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
  discord_account_id text NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  client_id text NOT NULL,
  guild_ids text[] NOT NULL CHECK (cardinality(guild_ids) BETWEEN 1 AND 100),
  scopes text[] NOT NULL CHECK (cardinality(scopes) BETWEEN 1 AND 6),
  created_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz,
  revoked_at timestamptz
);
CREATE INDEX mcp_connections_user ON mcp_connections(user_id);
-- Discord の連携を削除してから同じアカウントを再連携しても、以前の許可は復活しない。
ALTER TABLE "oauthRefreshToken" ADD CONSTRAINT mcp_refresh_connection FOREIGN KEY ("referenceId") REFERENCES mcp_connections(id) ON DELETE CASCADE;
ALTER TABLE "oauthConsent" ADD CONSTRAINT mcp_consent_connection FOREIGN KEY ("referenceId") REFERENCES mcp_connections(id) ON DELETE CASCADE;
