import { requireMcpAuth } from "@better-auth/mcp";
import { auth } from "../auth";
import { mcpEnabled, mcpResource, noStore } from "./config";
import { boundedRequest, privateResponse } from "./http";
import { activeConnection, authPool, type McpConnection } from "./store";

type TokenClaims = {
  sub?: string;
  azp?: unknown;
  client_id?: unknown;
  connection_id?: unknown;
  scope?: unknown;
  exp?: number;
  iss?: string;
  aud?: string | string[];
};
export function connectionMatches(
  connection: McpConnection | undefined,
  claims: TokenClaims,
) {
  return (
    !!connection &&
    connection.revoked_at === null &&
    claims.sub === connection.user_id &&
    claims.connection_id === connection.id &&
    claims.azp === connection.client_id &&
    typeof claims.scope === "string"
  );
}

export async function protectedMcp(
  request: Request,
  handler: (
    request: Request,
    connection: McpConnection,
    claims: TokenClaims,
  ) => Promise<Response>,
) {
  if (!mcpEnabled())
    return new Response(null, { status: 404, headers: noStore });
  let bounded: Request;
  try {
    bounded = await boundedRequest(request, 65_536);
  } catch {
    return new Response(null, { status: 413, headers: noStore });
  }
  const verify = requireMcpAuth(
    auth,
    async (req, claims) => {
      const connection =
        typeof claims.connection_id === "string" && claims.sub
          ? await activeConnection(claims.connection_id, claims.sub)
          : undefined;
      if (!connectionMatches(connection, claims) || !connection) {
        return Response.json(
          { error: "invalid_token" },
          {
            status: 401,
            headers: {
              ...noStore,
              "WWW-Authenticate": 'Bearer error="invalid_token"',
            },
          },
        );
      }
      await authPool.query(
        "UPDATE mcp_connections SET last_used_at = now() WHERE id = $1",
        [connection.id],
      );
      return handler(req, connection, {
        ...claims,
        scope: (claims.scope as string)
          .split(" ")
          .filter((s) => connection.scopes.includes(s))
          .join(" "),
      });
    },
    { resource: mcpResource() },
  );
  return privateResponse(await verify(bounded));
}
