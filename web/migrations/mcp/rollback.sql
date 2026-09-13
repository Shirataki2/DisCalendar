-- MCP_ENABLED=false にして新規認可・更新・実行を停止してから、検証 DB にのみ適用する。
-- MCP の接続と鍵をすべて削除する。再導入時は再接続が必要。既存 Web の認証テーブルは残す。
BEGIN;
DROP TABLE "oauthConsent", "oauthAccessToken", "oauthRefreshToken", "oauthClientAssertion", "oauthClientResource", "oauthResource", "oauthClient", jwks;
DROP TABLE mcp_connections;
COMMIT;
