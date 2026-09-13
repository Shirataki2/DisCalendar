// #216: 本体の DB・認証設定を使わず、固定版パッケージの接続前提を検証する。
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { cimd } from "@better-auth/cimd";
import { fetchClientMetadataResource } from "@better-auth/cimd/node";
import { mcp, requireMcpAuth } from "@better-auth/mcp";
import { createMcpHandler, McpServer } from "@modelcontextprotocol/server";
import { betterAuth } from "better-auth";
import { memoryAdapter } from "better-auth/adapters/memory";
import { getAuthTables } from "better-auth/db";
import { jwt } from "better-auth/plugins";

// 待受ポートは開かない。Request / Response をプロセス内で受け渡す。
const origin = "http://127.0.0.1:3216";
const resource = `${origin}/mcp`;
const scopes = [
  "guilds:read", "events:read", "events:create", "events:update", "events:delete",
];
const options = {
  baseURL: origin,
  secret: randomBytes(32).toString("hex"),
  plugins: [
    jwt(),
    mcp({
      resource,
      loginPage: "/login",
      consentPage: "/consent",
      scopes: [...scopes, "offline_access"],
    }),
    cimd({ fetchClientMetadataResource, metadataProfile: "mcp-2026-07-28" }),
  ],
};
const database = Object.fromEntries(
  Object.entries(getAuthTables(options)).map(([name, table]) => [
    table.modelName ?? name, [],
  ]),
);
const auth = betterAuth({ ...options, database: memoryAdapter(database) });

const protectedMetadata = await auth.handler(
  new Request(`${origin}/.well-known/oauth-protected-resource/mcp`),
);
assert.equal(protectedMetadata.status, 200);
const resourceMetadata = await protectedMetadata.json();
assert.equal(resourceMetadata.resource, resource);
assert.deepEqual(resourceMetadata.authorization_servers, [`${origin}/api/auth`]);
assert.deepEqual(resourceMetadata.scopes_supported, scopes);

const authorizationMetadata = await auth.handler(
  new Request(`${origin}/.well-known/oauth-authorization-server/api/auth`),
);
assert.equal(authorizationMetadata.status, 200);
const metadata = await authorizationMetadata.json();
assert.equal(metadata.issuer, `${origin}/api/auth`);
assert.equal(metadata.client_id_metadata_document_supported, true);
assert.equal(metadata.authorization_response_iss_parameter_supported, true);
assert.ok(metadata.token_endpoint_auth_methods_supported.includes("none"));
assert.deepEqual(metadata.code_challenge_methods_supported, ["S256"]);
assert.equal(metadata.registration_endpoint, undefined);
assert.equal(metadata.introspection_endpoint, `${origin}/api/auth/oauth2/introspect`);

let handlerCalls = 0;
const handler = createMcpHandler(
  () => new McpServer({ name: "discalendar-poc", version: "0.0.0" }),
);
const protectedHandler = requireMcpAuth(auth, (request) => {
  handlerCalls += 1;
  return handler.fetch(request);
}, { resource });
for (const authorization of [undefined, "Bearer invalid-test-token"]) {
  const headers = { "Content-Type": "application/json" };
  if (authorization) headers.Authorization = authorization;
  const response = await protectedHandler(new Request(resource, {
    method: "POST",
    headers,
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/list" }),
  }));
  assert.equal(response.status, 401);
  assert.match(response.headers.get("WWW-Authenticate"), /resource_metadata/);
}
assert.equal(handlerCalls, 0);
console.log("PASS: metadata / CIMD / PKCE S256 / DCR 非公開 / 未認証・不正トークン拒否");
console.log("未検証: Codex 実接続、Discord ログイン、同意、トークン発行・失効、Rust 認可");
