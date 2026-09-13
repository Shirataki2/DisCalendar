import { auth } from "../auth";
import {
  mcpConnectionsEnabled,
  mcpEnabled,
  mcpOrigin,
  mcpRequestAllowed,
  noStore,
} from "./config";
import {
  activeConnection,
  authPool,
  hashOAuthToken,
  revokeConnections,
} from "./store";

export async function boundedRequest(request: Request, max = 16_384) {
  if (request.url.length > max) throw new Error("Request too large");
  if (!request.body) return request;
  const reader = request.body.getReader();
  const chunks: Uint8Array[] = [];
  let size = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > max) {
      await reader.cancel();
      throw new Error("Request too large");
    }
    chunks.push(value);
  }
  return new Request(request.url, {
    method: request.method,
    headers: request.headers,
    body: new Blob(chunks as BlobPart[]),
  });
}

export function privateResponse(response: Response) {
  const result = new Response(response.body, response);
  for (const [key, value] of Object.entries(noStore))
    result.headers.set(key, value);
  return result;
}

export async function oauthHandler(input: Request) {
  let path: string;
  try {
    path = decodeURIComponent(new URL(input.url).pathname).replace(/\/+$/, "");
  } catch {
    return new Response(null, { status: 400, headers: noStore });
  }
  const oauth =
    path.startsWith("/.well-known/") ||
    path.startsWith("/api/auth/oauth2/") ||
    path === "/api/auth/jwks";
  if (oauth && !mcpEnabled())
    return new Response(null, { status: 404, headers: noStore });
  if (oauth && !mcpRequestAllowed(input))
    return new Response(null, { status: 403, headers: noStore });
  if (path === "/api/auth/oauth2/authorize" && !mcpConnectionsEnabled())
    return new Response("新規接続は現在停止中です。", {
      status: 503,
      headers: noStore,
    });
  // 同意はサーバー選択を検証する専用エンドポイントからのみ呼ぶ。
  if (
    path === "/api/auth/oauth2/consent" ||
    path === "/api/auth/oauth2/continue"
  ) {
    return new Response(null, { status: 403, headers: noStore });
  }
  let request: Request;
  try {
    request = await boundedRequest(input);
  } catch {
    return new Response(null, { status: 413, headers: noStore });
  }
  if (mcpEnabled() && path === "/api/auth/oauth2/token") {
    const raw = await request.clone().text();
    let body: Record<string, unknown>;
    try {
      body = request.headers
        .get("content-type")
        ?.toLowerCase()
        .includes("application/json")
        ? JSON.parse(raw)
        : Object.fromEntries(new URLSearchParams(raw));
      if (!body || typeof body !== "object") throw new Error("Invalid body");
    } catch {
      return Response.json(
        { error: "invalid_request" },
        { status: 400, headers: noStore },
      );
    }
    if (body.grant_type === "authorization_code" && !mcpConnectionsEnabled()) {
      return Response.json(
        { error: "temporarily_unavailable" },
        { status: 503, headers: noStore },
      );
    }
    if (body.grant_type === "refresh_token") {
      const { rows } = await authPool.query<{
        referenceId: string;
        userId: string;
        clientId: string;
        revoked: Date | null;
        rotationReplayExpiresAt: Date | null;
      }>(
        `SELECT "referenceId", "userId", "clientId", revoked, "rotationReplayExpiresAt" FROM "oauthRefreshToken" WHERE token = $1`,
        [hashOAuthToken(String(body.refresh_token ?? ""))],
      );
      const token = rows[0];
      if (
        !token ||
        !(await activeConnection(token.referenceId, token.userId))
      ) {
        return Response.json(
          { error: "invalid_grant" },
          { status: 400, headers: noStore },
        );
      }
      if (
        token.revoked &&
        (!token.rotationReplayExpiresAt ||
          token.rotationReplayExpiresAt < new Date())
      ) {
        // プラグインの猶予外再利用は利用者・クライアント全体を失効する。JWT にも同じ範囲を反映する。
        await revokeConnections(token.userId, undefined, token.clientId);
        return Response.json(
          { error: "invalid_grant" },
          { status: 400, headers: noStore },
        );
      }
    }
  }
  return privateResponse(await auth.handler(request));
}

export function sameOrigin(request: Request) {
  return request.headers.get("origin") === mcpOrigin();
}
