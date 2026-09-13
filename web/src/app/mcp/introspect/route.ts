import { timingSafeEqual } from "node:crypto";
import { mcpEnabled, noStore } from "@/lib/mcp/config";
import { boundedRequest } from "@/lib/mcp/http";
import { protectedMcp } from "@/lib/mcp/protected";

export const runtime = "nodejs";
export async function POST(request: Request) {
  if (!mcpEnabled())
    return new Response(null, { status: 404, headers: noStore });
  const secret = process.env.MCP_INTROSPECTION_SECRET;
  const supplied = Buffer.from(request.headers.get("authorization") ?? "");
  const expected = Buffer.from(`Bearer ${secret}`);
  if (
    !secret ||
    secret.length < 32 ||
    supplied.length !== expected.length ||
    !timingSafeEqual(supplied, expected)
  ) {
    return new Response(null, { status: 401, headers: noStore });
  }
  let token: unknown;
  try {
    token = (await (await boundedRequest(request)).json()).token;
  } catch {
    return new Response(null, { status: 400, headers: noStore });
  }
  if (typeof token !== "string" || !token || token.length > 8192)
    return new Response(null, { status: 400, headers: noStore });
  const headers = new Headers({ Authorization: `Bearer ${token}` });
  const response = await protectedMcp(
    new Request(request.url, { headers }),
    async (_req, connection, claims) =>
      Response.json(
        {
          active: true,
          sub: connection.user_id,
          client_id: connection.client_id,
          connection_id: connection.id,
          iss: claims.iss,
          aud: claims.aud,
          exp: claims.exp,
          scope: claims.scope,
          guild_ids: connection.guild_ids,
        },
        { headers: noStore },
      ),
  );
  if (response.status === 401 || response.status === 403)
    return Response.json({ active: false }, { headers: noStore });
  return response;
}
