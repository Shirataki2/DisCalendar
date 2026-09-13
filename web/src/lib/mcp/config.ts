export const MCP_SCOPES = {
  "guilds:read": "サーバー一覧の読み取り",
  "events:read": "予定の読み取り",
  "events:create": "予定の作成",
  "events:update": "予定の変更",
  "events:delete": "予定の削除",
  offline_access: "接続を継続して利用",
} as const;

export const mcpEnabled = () => process.env.MCP_ENABLED === "true";
export function mcpOrigin() {
  const url = new URL(process.env.BETTER_AUTH_URL ?? "http://localhost:3000");
  if (
    url.pathname !== "/" ||
    url.search ||
    url.hash ||
    url.username ||
    url.password
  ) {
    throw new Error("BETTER_AUTH_URL must be an origin for MCP");
  }
  return url.origin;
}
export const mcpResource = () => `${mcpOrigin()}/mcp`;
export const noStore = {
  "Cache-Control": "no-store",
  "Referrer-Policy": "strict-origin",
};

export function consentSelection(
  scopes: string[],
  guildIds: string[],
  requested: string[],
  available: string[],
) {
  if (
    !scopes.length ||
    scopes.length > 6 ||
    new Set(scopes).size !== scopes.length ||
    !scopes.every(
      (s) => Object.hasOwn(MCP_SCOPES, s) && requested.includes(s),
    ) ||
    !guildIds.length ||
    guildIds.length > 100 ||
    new Set(guildIds).size !== guildIds.length ||
    !guildIds.every((id) => /^\d{1,20}$/.test(id) && available.includes(id))
  ) {
    throw new Error("許可する権限とサーバーを選び直してください。");
  }
  return { scopes, guildIds };
}
