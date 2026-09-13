import { createHash, randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { afterAll, beforeAll, expect, test, vi } from "vitest";
import { signSessionCookie } from "../../../e2e/seed";
import { accountIssuerCompatibility } from "../auth-schema.mjs";

const mocks = vi.hoisted(() => ({
  headers: new Headers(),
  guilds: [{ id: "123456789012345678", name: "検証サーバー" }],
}));
vi.mock("next/headers", () => ({ headers: async () => mocks.headers }));
vi.mock("../discord", () => ({ getUserGuilds: async () => mocks.guilds }));
vi.mock("../joined-guilds", () => ({
  loadJoinedGuildIds: async () => ({
    ok: true,
    ids: new Set(mocks.guilds.map((g) => g.id)),
  }),
}));
vi.mock("@better-auth/cimd/node", () => ({
  fetchClientMetadataResource: async () =>
    Response.json({
      client_id: "https://client.example/metadata.json",
      client_name: "検証クライアント",
      redirect_uris: ["http://127.0.0.1/callback"],
      token_endpoint_auth_method: "none",
      grant_types: ["authorization_code", "refresh_token"],
      response_types: ["code"],
      application_type: "native",
    }),
}));

// 明示指定の専用 DB だけを使う。通常の pnpm test では DB を必要としない。
const databaseUrl = process.env.MCP_TEST_DATABASE_URL;
const suite = databaseUrl ? test : test.skip;
const origin = "http://127.0.0.1:35217";
const resource = `${origin}/mcp`;
const clientId = "https://client.example/metadata.json";
const secret = randomBytes(32).toString("hex");
let auth: typeof import("../auth").auth;
let store: typeof import("./store");
let http: typeof import("./http");
let submit: typeof import("../../app/mcp/consent/submit/route").POST;
let protectedMcp: typeof import("./protected").protectedMcp;
let cookie: string;

beforeAll(async () => {
  if (!databaseUrl) return;
  const url = new URL(databaseUrl);
  if (url.hostname !== "127.0.0.1" || url.pathname !== "/mcp_217_test")
    throw new Error("専用のローカル検証 DB が必要です");
  vi.stubEnv("DATABASE_URL", databaseUrl);
  vi.stubEnv("BETTER_AUTH_URL", origin);
  vi.stubEnv("BETTER_AUTH_SECRET", secret);
  vi.stubEnv("MCP_ENABLED", "true");
  vi.stubEnv("DISCORD_CLIENT_ID", "mcp-test");
  vi.stubEnv("DISCORD_CLIENT_SECRET", randomBytes(32).toString("hex"));
  store = await import("./store");
  const { getMigrations } = await import("better-auth/db/migration");
  await store.authPool.query(
    "DROP SCHEMA public CASCADE; CREATE SCHEMA public",
  );
  await (
    await getMigrations({
      database: store.authPool,
      plugins: [accountIssuerCompatibility],
    })
  ).runMigrations();
  await store.authPool.query(
    readFileSync("migrations/mcp/001_oauth.sql", "utf8"),
  );
  await store.authPool.query(
    readFileSync("migrations/mcp/002_connections.sql", "utf8"),
  );
  // 1.7.2 と同じ、DB既定値のない NOT NULL 列でも新規アカウントを書けることを確認する。
  await store.authPool.query(
    `ALTER TABLE account ALTER COLUMN issuer DROP DEFAULT`,
  );
  auth = (await import("../auth")).auth;
  http = await import("./http");
  submit = (await import("../../app/mcp/consent/submit/route")).POST;
  protectedMcp = (await import("./protected")).protectedMcp;
  const migrations = await getMigrations(auth.options);
  expect(migrations.toBeCreated).toEqual([]);
  expect(migrations.toBeAdded).toEqual([]);
  const ctx = await auth.$context;
  await ctx.internalAdapter.createUser(
    {
      id: "mcp-user",
      name: "検証利用者",
      email: "mcp@example.test",
      emailVerified: true,
    },
    { method: "oauth", oauth: { providerId: "discord", profile: {} } },
  );
  await ctx.internalAdapter.createAccount({
    id: "mcp-discord",
    userId: "mcp-user",
    providerId: "discord",
    accountId: "111111111111111111",
  });
  expect(
    (
      await store.authPool.query(
        `SELECT issuer FROM account WHERE id = 'mcp-discord'`,
      )
    ).rows[0].issuer,
  ).toBe("local:oauth:discord");
  const session = await ctx.internalAdapter.createSession("mcp-user", false);
  cookie = `better-auth.session_token=${signSessionCookie(session.token, secret)}`;
  vi.stubGlobal(
    "fetch",
    async (input: RequestInfo | URL, init?: RequestInit) => {
      const request =
        input instanceof Request ? input : new Request(input, init);
      if (request.url === `${origin}/api/auth/jwks`)
        return auth.handler(request);
      if (request.url === "https://discord.com/api/oauth2/token")
        return Response.json({
          access_token: randomBytes(24).toString("hex"),
          refresh_token: randomBytes(24).toString("hex"),
          token_type: "Bearer",
          expires_in: 3600,
          scope: "identify email guilds",
        });
      if (
        decodeURIComponent(request.url) === "https://discord.com/api/users/@me"
      )
        return Response.json({
          id: "111111111111111111",
          username: "検証利用者",
          global_name: "検証利用者",
          email: "mcp@example.test",
          verified: true,
          avatar: null,
          discriminator: "0",
        });
      throw new Error(
        `Unexpected fetch path: ${new URL(request.url).pathname}`,
      );
    },
  );
}, 30_000);

afterAll(async () => {
  if (store) await store.authPool.end();
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

function request(path: string, body?: URLSearchParams, signedIn = true) {
  const headers = new Headers({ Origin: origin, Host: new URL(origin).host });
  if (signedIn) headers.set("Cookie", cookie);
  if (body) headers.set("Content-Type", "application/x-www-form-urlencoded");
  mocks.headers = headers;
  return new Request(`${origin}${path}`, {
    method: body ? "POST" : "GET",
    headers,
    body,
  });
}
async function authorize(
  scope = "guilds:read offline_access",
  signedIn = true,
  overrides: Record<string, string> = {},
) {
  const verifier = randomBytes(32).toString("base64url");
  const params = new URLSearchParams({
    client_id: clientId,
    redirect_uri: "http://127.0.0.1:45678/callback",
    response_type: "code",
    scope,
    resource,
    code_challenge: createHash("sha256").update(verifier).digest("base64url"),
    code_challenge_method: "S256",
    state: randomBytes(16).toString("hex"),
    ...overrides,
  });
  const response = await http.oauthHandler(
    request(`/api/auth/oauth2/authorize?${params}`, undefined, signedIn),
  );
  const location = response.headers.get("location");
  return { response, location, verifier, params };
}
async function consent(
  query: string,
  scopes = ["guilds:read", "offline_access"],
  guildId = mocks.guilds[0].id,
) {
  const form = new URLSearchParams({
    oauth_query: query,
    accept: "true",
    guild_id: guildId,
  });
  for (const scope of scopes) form.append("scope", scope);
  return submit(request("/mcp/consent/submit", form));
}
async function issue(scopes = ["guilds:read", "offline_access"]) {
  const flow = await authorize(scopes.join(" "));
  expect(flow.location, await flow.response.clone().text()).toContain(
    "/mcp/consent?",
  );
  const response = await consent(
    new URL(flow.location ?? "", origin).search.slice(1),
    scopes,
  );
  expect(response.status, await response.clone().text()).toBe(303);
  const callback = new URL(response.headers.get("location") ?? "");
  expect(callback.searchParams.get("state")).toBe(flow.params.get("state"));
  expect(callback.searchParams.get("iss")).toBe(`${origin}/api/auth`);
  const exchange = new URLSearchParams({
    grant_type: "authorization_code",
    client_id: clientId,
    redirect_uri: flow.params.get("redirect_uri") ?? "",
    resource,
    code: callback.searchParams.get("code") ?? "",
    code_verifier: flow.verifier,
  });
  const result = await http.oauthHandler(
    request("/api/auth/oauth2/token", exchange, false),
  );
  const tokens = await result.json();
  expect(result.status, JSON.stringify(tokens)).toBe(200);
  expect(tokens.expires_in).toBe(900);
  return { tokens, exchange };
}
async function read(token: string) {
  return protectedMcp(
    new Request(resource, {
      headers: { Host: new URL(origin).host, Authorization: `Bearer ${token}` },
    }),
    async (_req, connection) =>
      Response.json({ id: connection.id, guild_ids: connection.guild_ids }),
  );
}
async function refresh(
  token: string,
  requestedClientId: string | null = clientId,
) {
  return http.oauthHandler(
    request(
      "/api/auth/oauth2/token",
      new URLSearchParams({
        grant_type: "refresh_token",
        ...(requestedClientId === null ? {} : { client_id: requestedClientId }),
        refresh_token: token,
        resource,
      }),
      false,
    ),
  );
}

suite(
  "metadata・CIMD・loopback・初回同意・PKCE交換・読み取り・コード再利用拒否",
  async () => {
    const metadata = await http.oauthHandler(
      request(
        "/.well-known/oauth-authorization-server/api/auth",
        undefined,
        false,
      ),
    );
    const json = await metadata.json();
    expect(json.client_id_metadata_document_supported).toBe(true);
    expect(json.registration_endpoint).toBeUndefined();
    expect(json.code_challenge_methods_supported).toEqual(["S256"]);
    const login = await authorize(undefined, false);
    expect(login.location).toContain("/mcp/login?");
    const { tokens, exchange } = await issue();
    const response = await read(tokens.access_token);
    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(
      (
        await http.oauthHandler(
          request("/api/auth/oauth2/token", exchange, false),
        )
      ).status,
    ).toBe(400);
  },
);

suite(
  "OAuth専用ログインはDiscordのstate経由で同意へ復帰し、改ざんを拒否する",
  async () => {
    const flow = await authorize(undefined, false);
    const signedQuery = new URL(flow.location ?? "", origin).search.slice(1);
    const start = (await import("../../app/mcp/login/start/route")).POST;
    const loginRequest = (query: string) =>
      new Request(`${origin}/mcp/login/start`, {
        method: "POST",
        headers: { Origin: origin, "Content-Type": "application/json" },
        body: JSON.stringify({ oauth_query: query }),
      });
    expect(
      (await start(loginRequest(`${signedQuery}&scope=events:delete`))).status,
    ).toBe(400);
    const login = await start(loginRequest(signedQuery));
    expect(login.status).toBe(200);
    const discord = new URL((await login.json()).url);
    expect(discord.origin).toBe("https://discord.com");
    const state = discord.searchParams.get("state") ?? "";
    const cookies = login.headers
      .getSetCookie()
      .map((c) => c.split(";")[0])
      .join("; ");
    const callback = await auth.handler(
      new Request(
        `${origin}/api/auth/callback/discord?${new URLSearchParams({ state, code: "mock-discord-code" })}`,
        { headers: { Cookie: cookies, Accept: "text/html" } },
      ),
    );
    expect(
      callback.headers.get("location"),
      await callback.clone().text(),
    ).toContain("/mcp/consent?");
  },
);

suite("scope・サーバー・署名改ざんと未同意・誤PKCEを拒否", async () => {
  const flow = await authorize();
  const query = new URL(flow.location ?? "", origin).search.slice(1);
  expect((await consent(query, ["events:delete"])).status).toBe(400);
  expect((await consent(query, undefined, "999")).status).toBe(400);
  expect((await consent(`${query}&resource=https://evil.example`)).status).toBe(
    400,
  );
  const result = await consent(query);
  const code =
    new URL(result.headers.get("location") ?? "").searchParams.get("code") ??
    "";
  expect(
    (
      await http.oauthHandler(
        request(
          "/api/auth/oauth2/token",
          new URLSearchParams({
            grant_type: "authorization_code",
            client_id: clientId,
            redirect_uri: "http://127.0.0.1:45678/callback",
            code,
            code_verifier: "wrong",
          }),
          false,
        ),
      )
    ).status,
  ).toBe(401);
  expect((await read("invalid")).status).toBe(401);
  expect(
    (
      await http.oauthHandler(
        request(
          "/api/auth/oauth2/consent",
          new URLSearchParams({ accept: "true", oauth_query: query }),
        ),
      )
    ).status,
  ).toBe(403);
});

suite(
  "再同意で旧トークンを拡張せず、解除後は読み取り・更新を拒否",
  async () => {
    const first = await issue();
    const second = await issue([
      "guilds:read",
      "events:delete",
      "offline_access",
    ]);
    const old = await (await read(first.tokens.access_token)).json();
    const newer = await (await read(second.tokens.access_token)).json();
    expect(old.id).not.toBe(newer.id);
    await store.revokeConnections("mcp-user", old.id);
    expect((await read(first.tokens.access_token)).status).toBe(401);
    expect((await refresh(first.tokens.refresh_token)).status).toBe(400);
    expect((await read(second.tokens.access_token)).status).toBe(200);
  },
);

suite.each([clientId, null, "https://other.example/client.json"])(
  "更新の並行利用・猶予外再利用の失効（client_id=%s）",
  async (requestedClientId) => {
    const { tokens } = await issue();
    const second = await issue();
    const results = await Promise.all([
      refresh(tokens.refresh_token),
      refresh(tokens.refresh_token),
    ]);
    expect(results.some((r) => r.status === 200)).toBe(true);
    expect(results.every((r) => r.status === 200 || r.status === 400)).toBe(
      true,
    );
    const success = await results.find((r) => r.status === 200)?.json();
    const replay = await refresh(tokens.refresh_token);
    expect(replay.status).toBe(200);
    expect((await replay.json()).refresh_token).toBe(success.refresh_token);
    await store.authPool.query(
      `UPDATE "oauthRefreshToken" SET "rotationReplayExpiresAt" = now() - interval '1 second' WHERE token = $1`,
      [store.hashOAuthToken(tokens.refresh_token)],
    );
    expect(
      (await refresh(tokens.refresh_token, requestedClientId)).status,
    ).toBe(400);
    expect((await read(second.tokens.access_token)).status).toBe(401);
    expect((await refresh(second.tokens.refresh_token)).status).toBe(400);
    expect((await read(success.access_token)).status).toBe(401);
    expect((await refresh(success.refresh_token)).status).toBe(400);
  },
);

suite(
  "期限切れ・誤issuer/audience・接続のないJWT・introspectionの認証を拒否",
  async () => {
    const { tokens } = await issue();
    const claims = JSON.parse(
      Buffer.from(tokens.access_token.split(".")[1], "base64url").toString(),
    );
    for (const invalid of [
      { exp: 1 },
      { iss: "https://wrong.example" },
      { aud: "https://wrong.example/mcp" },
      { connection_id: "missing" },
    ]) {
      const { token } = await auth.api.signJWT({
        body: { payload: { ...claims, ...invalid } },
      });
      expect((await read(token)).status).toBe(401);
    }
    const introspect = (await import("../../app/mcp/introspect/route")).POST;
    vi.stubEnv("MCP_INTROSPECTION_SECRET", randomBytes(32).toString("hex"));
    const call = (credential: string) =>
      introspect(
        new Request(`${origin}/mcp/introspect`, {
          method: "POST",
          headers: {
            Authorization: `Bearer ${credential}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ token: tokens.access_token }),
        }),
      );
    expect((await call("wrong")).status).toBe(401);
    const checked = await call(process.env.MCP_INTROSPECTION_SECRET ?? "");
    expect((await checked.json()).active).toBe(true);
    vi.stubEnv("MCP_ENABLED", "false");
    expect((await read(tokens.access_token)).status).toBe(404);
    expect((await refresh(tokens.refresh_token)).status).toBe(404);
    expect((await authorize()).response.status).toBe(404);
    expect(
      (
        await http.oauthHandler(
          request("/.well-known/oauth-protected-resource/mcp"),
        )
      ).status,
    ).toBe(404);
    vi.stubEnv("MCP_ENABLED", "true");
  },
);

suite("MCPとOAuthは偽装Host・外部Originを拒否してno-storeを返す", async () => {
  const post = (await import("../../app/mcp/route")).POST;
  for (const headers of [
    new Headers({ Host: "other.example" }),
    new Headers({
      Host: new URL(origin).host,
      Origin: "https://other.example",
    }),
    new Headers({ Host: new URL(origin).host, Origin: "null" }),
  ]) {
    const mcpResponse = await post(
      new Request(resource, { method: "POST", headers }),
    );
    const oauthResponse = await http.oauthHandler(
      new Request(`${origin}/.well-known/oauth-protected-resource/mcp`, {
        headers,
      }),
    );
    for (const response of [mcpResponse, oauthResponse]) {
      expect(response.status).toBe(403);
      expect(response.headers.get("cache-control")).toBe("no-store");
    }
  }
});

suite(
  "新規接続停止中も既存接続の読み取り・更新・解除とWebセッションを維持する",
  async () => {
    const { tokens } = await issue();
    vi.stubEnv("MCP_NEW_CONNECTIONS_ENABLED", "false");
    try {
      expect((await authorize()).response.status).toBe(503);
      expect(
        (
          await http.oauthHandler(
            request(
              "/api/auth/oauth2/token",
              new URLSearchParams({ grant_type: "authorization_code" }),
            ),
          )
        ).status,
      ).toBe(503);
      expect(
        (await submit(request("/mcp/consent/submit", new URLSearchParams())))
          .status,
      ).toBe(404);
      expect((await read(tokens.access_token)).status).toBe(200);
      expect((await refresh(tokens.refresh_token)).status).toBe(200);
      expect(
        await auth.api.getSession({ headers: request("/").headers }),
      ).not.toBeNull();
      const claims = JSON.parse(
        Buffer.from(tokens.access_token.split(".")[1], "base64url").toString(),
      );
      const revoke = (await import("../../app/mcp/connections/revoke/route"))
        .POST;
      expect(
        (
          await revoke(
            request(
              "/mcp/connections/revoke",
              new URLSearchParams({ id: claims.connection_id }),
            ),
          )
        ).status,
      ).toBe(303);
      expect((await read(tokens.access_token)).status).toBe(401);
    } finally {
      vi.stubEnv("MCP_NEW_CONNECTIONS_ENABLED", "true");
    }
  },
);

suite(
  "公式SDKのinitialize・tools/list・connection_infoを認証付きで呼べる",
  async () => {
    const { tokens } = await issue();
    const post = (await import("../../app/mcp/route")).POST;
    const call = (method: string, params: object) =>
      post(
        new Request(resource, {
          method: "POST",
          headers: {
            Host: new URL(origin).host,
            Authorization: `Bearer ${tokens.access_token}`,
            "Content-Type": "application/json",
            Accept: "application/json, text/event-stream",
            "MCP-Protocol-Version": "2025-11-25",
          },
          body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
        }),
      );
    const initialized = await call("initialize", {
      protocolVersion: "2025-11-25",
      capabilities: {},
      clientInfo: { name: "integration-test", version: "1" },
    });
    expect(initialized.status, await initialized.clone().text()).toBe(200);
    expect(await initialized.text()).toContain("2025-11-25");
    const listed = await call("tools/list", {});
    expect(listed.status).toBe(200);
    expect(await listed.text()).toContain("connection_info");
    const result = await call("tools/call", {
      name: "connection_info",
      arguments: {},
    });
    expect(result.status).toBe(200);
    expect(await result.text()).toContain("guild_ids");
  },
);

suite(
  "現在のscopeを縮小すると既存トークンの実効scopeも次の操作から縮小する",
  async () => {
    const { tokens } = await issue([
      "guilds:read",
      "events:read",
      "offline_access",
    ]);
    const claims = JSON.parse(
      Buffer.from(tokens.access_token.split(".")[1], "base64url").toString(),
    );
    await store.authPool.query(
      "UPDATE mcp_connections SET scopes = $1 WHERE id = $2",
      [["guilds:read"], claims.connection_id],
    );
    const response = await protectedMcp(
      new Request(resource, {
        headers: {
          Host: new URL(origin).host,
          Authorization: `Bearer ${tokens.access_token}`,
        },
      }),
      async (_req, _connection, effective) =>
        Response.json({ scope: effective.scope }),
    );
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ scope: "guilds:read" });
  },
);

suite("Webログアウトで維持し、Discord連携削除で失効", async () => {
  const { tokens } = await issue();
  await auth.handler(request("/api/auth/sign-out", new URLSearchParams()));
  expect((await read(tokens.access_token)).status).toBe(200);
  expect((await refresh(tokens.refresh_token)).status).toBe(200);
  await store.authPool.query(`DELETE FROM account WHERE id = 'mcp-discord'`);
  expect((await read(tokens.access_token)).status).toBe(401);
  expect((await refresh(tokens.refresh_token)).status).toBe(400);
});

suite(
  "アカウント削除で失効し、rollback後も基本認証テーブルを維持する",
  async () => {
    const ctx = await auth.$context;
    await ctx.internalAdapter.createAccount({
      id: "mcp-discord-relinked",
      userId: "mcp-user",
      providerId: "discord",
      accountId: "111111111111111111",
    });
    const session = await ctx.internalAdapter.createSession("mcp-user", false);
    cookie = `better-auth.session_token=${signSessionCookie(session.token, secret)}`;
    const { tokens } = await issue();
    await store.authPool.query(`DELETE FROM "user" WHERE id = 'mcp-user'`);
    expect((await read(tokens.access_token)).status).toBe(401);
    expect((await refresh(tokens.refresh_token)).status).toBe(400);
    await store.authPool.query(
      readFileSync("migrations/mcp/rollback.sql", "utf8"),
    );
    const { rows } = await store.authPool.query(
      `SELECT to_regclass('mcp_connections') AS mcp, to_regclass('"user"') AS users`,
    );
    expect(rows[0]).toEqual({ mcp: null, users: '"user"' });
  },
);
