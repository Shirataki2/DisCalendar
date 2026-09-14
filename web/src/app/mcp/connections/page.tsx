import { headers } from "next/headers";
import { redirect } from "next/navigation";
import { McpForm } from "@/components/mcp-form";
import { Button } from "@/components/ui/button";
import { auth } from "@/lib/auth";
import { getUserGuilds } from "@/lib/discord";
import { MCP_SCOPES, mcpEnabled } from "@/lib/mcp/config";
import { authPool, type McpConnection } from "@/lib/mcp/store";

function formatDate(date: Date) {
  return `${new Intl.DateTimeFormat("ja-JP", {
    dateStyle: "medium",
    timeStyle: "short",
    timeZone: "Asia/Tokyo",
  }).format(date)} (日本時間)`;
}

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
    <main className="mx-auto w-full max-w-3xl space-y-6 px-4 py-8 sm:px-8">
      <h1 className="text-2xl font-bold">MCP 接続管理</h1>
      {!mcpEnabled() && <p>MCPは現在停止中です。既存の接続は解除できます。</p>}
      <p>
        解除後に開始した操作とトークンの更新を拒否します。実行中の操作は取り消せません。Webからのログアウトだけでは接続は解除されません。
      </p>
      {!connections.length && (
        <p className="rounded-xl border border-dashed border-border p-8 text-center text-muted-foreground">
          接続はありません。AIクライアントから接続すると、ここで許可した内容を確認・解除できます。
        </p>
      )}
      {connections.map((c) => (
        <article
          key={c.id}
          className="space-y-4 rounded-xl border border-border bg-card p-4 sm:p-6"
        >
          <h2 className="break-all font-semibold">{c.client_id}</h2>
          <dl className="grid gap-3 text-sm sm:grid-cols-[7rem_minmax(0,1fr)]">
            <dt className="text-muted-foreground">対象サーバー</dt>
            <dd className="break-words">
              {c.guild_ids.map((id) => guildNames.get(id) ?? id).join("、")}
            </dd>
            <dt className="text-muted-foreground">許可した操作</dt>
            <dd className="break-words">
              {c.scopes
                .map((s) => MCP_SCOPES[s as keyof typeof MCP_SCOPES] ?? s)
                .join("、")}
            </dd>
            <dt className="text-muted-foreground">接続日時</dt>
            <dd>
              <time dateTime={c.created_at.toISOString()}>
                {formatDate(c.created_at)}
              </time>
            </dd>
            <dt className="text-muted-foreground">最終利用</dt>
            <dd>
              {c.last_used_at ? (
                <time dateTime={c.last_used_at.toISOString()}>
                  {formatDate(c.last_used_at)}
                </time>
              ) : (
                "未使用"
              )}
            </dd>
          </dl>
          <McpForm action="/mcp/connections/revoke">
            <input type="hidden" name="id" value={c.id} />
            <Button type="submit" variant="destructive" size="lg">
              接続を解除
            </Button>
          </McpForm>
        </article>
      ))}
    </main>
  );
}
