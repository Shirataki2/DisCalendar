import { headers } from "next/headers";
import { redirect } from "next/navigation";
import { auth } from "@/lib/auth";
import { getUserGuilds } from "@/lib/discord";
import { MCP_SCOPES, mcpEnabled } from "@/lib/mcp/config";
import { authPool, type McpConnection } from "@/lib/mcp/store";

export default async function ConnectionsPage() {
  const session = await auth.api.getSession({ headers: await headers() });
  if (!session) redirect("/login");
  // 全停止中も解除できる。未導入環境ではテーブルに触れない。
  const exists = await authPool.query(
    "SELECT to_regclass('mcp_connections') AS name",
  );
  const connections = exists.rows[0].name
    ? (
        await authPool.query<McpConnection>(
          "SELECT * FROM mcp_connections WHERE user_id = $1 AND revoked_at IS NULL ORDER BY created_at DESC",
          [session.user.id],
        )
      ).rows
    : [];
  const guilds = connections.length
    ? await getUserGuilds().catch(() => [])
    : [];
  const guildNames = new Map(guilds.map((g) => [g.id, g.name]));
  return (
    <main className="mx-auto max-w-2xl space-y-6 p-8">
      <h1 className="text-2xl font-bold">MCP 接続管理</h1>
      {!mcpEnabled() && <p>MCPは現在停止中です。既存の接続は解除できます。</p>}
      <p>
        解除後に開始した操作とトークンの更新を拒否します。実行中の操作は取り消せません。Webからのログアウトだけでは接続は解除されません。
      </p>
      {!connections.length && <p>接続はありません。</p>}
      {connections.map((c) => (
        <article key={c.id} className="space-y-3 rounded border p-4">
          <h2 className="break-all font-semibold">{c.client_id}</h2>
          <p>
            対象サーバー:{" "}
            {c.guild_ids.map((id) => guildNames.get(id) ?? id).join("、")}
          </p>
          <p>
            権限:{" "}
            {c.scopes
              .map((s) => MCP_SCOPES[s as keyof typeof MCP_SCOPES] ?? s)
              .join("、")}
          </p>
          <p>接続日時: {c.created_at.toISOString()}</p>
          <p>最終利用: {c.last_used_at?.toISOString() ?? "未使用"}</p>
          <form action="/mcp/connections/revoke" method="post">
            <input type="hidden" name="id" value={c.id} />
            <button type="submit" className="rounded border px-4 py-2">
              接続を解除
            </button>
          </form>
        </article>
      ))}
    </main>
  );
}
