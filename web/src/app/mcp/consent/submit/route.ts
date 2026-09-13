import { randomUUID } from "node:crypto";
import { auth } from "@/lib/auth";
import { getUserGuilds } from "@/lib/discord";
import { loadJoinedGuildIds } from "@/lib/joined-guilds";
import {
  consentSelection,
  mcpConnectionsEnabled,
  noStore,
} from "@/lib/mcp/config";
import { boundedRequest, sameOrigin } from "@/lib/mcp/http";
import { authPool, consentContext } from "@/lib/mcp/store";

export async function POST(request: Request) {
  if (!mcpConnectionsEnabled())
    return new Response(null, { status: 404, headers: noStore });
  if (!sameOrigin(request))
    return new Response(null, { status: 403, headers: noStore });
  const session = await auth.api.getSession({ headers: request.headers });
  if (!session) return new Response(null, { status: 401, headers: noStore });
  let id: string | undefined;
  try {
    const form = await (await boundedRequest(request)).formData();
    const query = String(form.get("oauth_query") ?? "");
    const params = new URLSearchParams(query);
    const accept = form.get("accept") === "true";
    const clientId = params.get("client_id");
    if (!clientId || clientId.length > 2048) throw new Error("Invalid client");
    let scope: string | undefined;
    if (accept) {
      const guilds = await getUserGuilds();
      const joined = await loadJoinedGuildIds(guilds);
      if (!joined.ok) throw new Error("サーバー一覧を取得できませんでした。");
      const selected = consentSelection(
        form.getAll("scope").map(String),
        form.getAll("guild_id").map(String),
        (params.get("scope") ?? "").split(" "),
        guilds.filter((g) => joined.ids.has(g.id)).map((g) => g.id),
      );
      scope = selected.scopes.join(" ");
      id = randomUUID();
      const result = await authPool.query(
        `INSERT INTO mcp_connections (id, user_id, discord_account_id, client_id, scopes, guild_ids)
         SELECT $1, $2, id, $3, $4, $5 FROM account WHERE "userId" = $2 AND "providerId" = 'discord' LIMIT 1`,
        [id, session.user.id, clientId, selected.scopes, selected.guildIds],
      );
      if (result.rowCount !== 1) throw new Error("Discord account missing");
    }
    // プラグインが署名・期限・要求 scope を検証するまで、接続 ID は発行済みトークンへ渡らない。
    const result = await consentContext.run(
      { id: id ?? "", userId: session.user.id },
      () =>
        auth.handler(
          new Request(new URL("/api/auth/oauth2/consent", request.url), {
            method: "POST",
            headers: {
              cookie: request.headers.get("cookie") ?? "",
              origin: request.headers.get("origin") ?? "",
              "content-type": "application/json",
            },
            body: JSON.stringify({ accept, scope, oauth_query: query }),
          }),
        ),
    );
    const redirect = await result.json();
    if (
      !result.ok ||
      typeof redirect.url !== "string" ||
      (accept && !new URL(redirect.url).searchParams.has("code"))
    )
      throw new Error("Consent failed");
    return new Response(null, {
      status: 303,
      headers: { ...noStore, Location: redirect.url },
    });
  } catch {
    if (id)
      await authPool.query("DELETE FROM mcp_connections WHERE id = $1", [id]);
    return new Response(
      "接続を許可できませんでした。クライアントから接続をやり直してください。",
      { status: 400, headers: noStore },
    );
  }
}
