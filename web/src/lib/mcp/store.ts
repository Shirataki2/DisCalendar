import { AsyncLocalStorage } from "node:async_hooks";
import { createHash } from "node:crypto";
import { Pool } from "pg";

export const authPool = new Pool({
  connectionString: process.env.DATABASE_URL,
});
// 同意処理の内部呼び出しだけで共有する。HTTP ヘッダーや利用者の入力から直接設定しない。
export const consentContext = new AsyncLocalStorage<{
  id: string;
  userId: string;
}>();
export const hashOAuthToken = (token: string) =>
  createHash("sha256").update(token).digest("hex");

export interface McpConnection {
  id: string;
  user_id: string;
  client_id: string;
  guild_ids: string[];
  scopes: string[];
  created_at: Date;
  last_used_at: Date | null;
  revoked_at: Date | null;
}

export async function activeConnection(id: string, userId: string) {
  const { rows } = await authPool.query<McpConnection>(
    `SELECT c.* FROM mcp_connections c
     JOIN "oauthClient" client ON client."clientId" = c.client_id AND client.disabled IS DISTINCT FROM true
     JOIN "user" u ON u.id = c.user_id
     JOIN account a ON a.id = c.discord_account_id AND a."userId" = u.id AND a."providerId" = 'discord'
     WHERE c.id = $1 AND c.user_id = $2 AND c.revoked_at IS NULL`,
    [id, userId],
  );
  return rows[0];
}

export async function revokeConnections(
  userId: string,
  id?: string,
  clientId?: string,
) {
  // 先に許可を失効する。後続のトークン削除に失敗しても、実行・更新は失効レコードで拒否する。
  const { rows } = await authPool.query<{ id: string }>(
    `UPDATE mcp_connections SET revoked_at = COALESCE(revoked_at, now())
     WHERE user_id = $1 AND ($2::text IS NULL OR id = $2) AND ($3::text IS NULL OR client_id = $3) RETURNING id`,
    [userId, id ?? null, clientId ?? null],
  );
  if (rows.length) {
    await authPool.query(
      `DELETE FROM "oauthRefreshToken" WHERE "referenceId" = ANY($1::text[])`,
      [rows.map((r) => r.id)],
    );
    await authPool.query(
      `DELETE FROM "oauthConsent" WHERE "referenceId" = ANY($1::text[])`,
      [rows.map((r) => r.id)],
    );
  }
}
