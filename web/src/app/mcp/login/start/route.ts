import { auth } from "@/lib/auth";
import { mcpConnectionsEnabled, noStore } from "@/lib/mcp/config";
import { boundedRequest, privateResponse, sameOrigin } from "@/lib/mcp/http";

export async function POST(request: Request) {
  if (!mcpConnectionsEnabled())
    return new Response(null, { status: 404, headers: noStore });
  if (!sameOrigin(request))
    return new Response(null, { status: 403, headers: noStore });
  let body: { oauth_query?: unknown };
  try {
    body = await (await boundedRequest(request)).json();
  } catch {
    return new Response(null, { status: 400, headers: noStore });
  }
  if (typeof body.oauth_query !== "string")
    return new Response(null, { status: 400, headers: noStore });
  // 署名付き oauth_query はプラグインが検証し、Discord の state に保存して認可へ復帰する。
  return privateResponse(
    await auth.handler(
      new Request(new URL("/api/auth/sign-in/social", request.url), {
        method: "POST",
        headers: request.headers,
        body: JSON.stringify({
          provider: "discord",
          callbackURL: "/mcp/connections",
          oauth_query: body.oauth_query,
        }),
      }),
    ),
  );
}
