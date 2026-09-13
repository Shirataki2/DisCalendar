import { createMcpHandler, McpServer } from "@modelcontextprotocol/server";
import { afterEach, expect, test, vi } from "vitest";
import { registerWriteTools } from "./write-tools";

afterEach(() => vi.unstubAllGlobals());
const handler = createMcpHandler(() => {
  const server = new McpServer({ name: "test", version: "1" });
  registerWriteTools(
    server,
    new Request("http://localhost/mcp", {
      headers: { Authorization: "Bearer oauth-test", Cookie: "private-cookie" },
    }),
  );
  return server;
});
async function call(name: string, args: object) {
  const response = await handler.fetch(
    new Request("http://localhost/mcp", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
      },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "tools/call",
        params: { name, arguments: args },
      }),
    }),
  );
  const body = await response.text();
  return JSON.parse(
    body.startsWith("event:") ? body.split("data: ")[1].trim() : body,
  );
}

test("書き込みと照会はBearerのみを送り、副作用の結果を保持する", async () => {
  const data = { database: "succeeded", discord: "unknown", event_id: 1 };
  const fetcher = vi.fn(async () => Response.json(data));
  vi.stubGlobal("fetch", fetcher);
  const identity = { guild_id: "111", idempotency_key: "operation" };
  for (const [name, path, input] of [
    [
      "create_event",
      "create",
      {
        ...identity,
        changes: {
          name: "予定",
          color: "#123456",
          start_at: "2099-09-13T10:00:00+09:00",
          end_at: "2099-09-13T11:00:00+09:00",
        },
      },
    ],
    [
      "update_event",
      "update",
      {
        ...identity,
        event_id: 1,
        expected_version: "v",
        changes: { description: null },
      },
    ],
    [
      "delete_event",
      "delete",
      { ...identity, event_id: 1, expected_version: "v" },
    ],
    ["get_event_operation", "operation", identity],
  ] as const) {
    expect((await call(name, input)).result.structuredContent).toEqual(data);
    expect(fetcher).toHaveBeenLastCalledWith(
      expect.stringContaining(`/mcp/events/${path}`),
      expect.objectContaining({
        headers: {
          Authorization: "Bearer oauth-test",
          "Content-Type": "application/json",
        },
        body: JSON.stringify(input),
        cache: "no-store",
        redirect: "error",
      }),
    );
  }
});

test("版の省略・未知の引数・利用者偽装を拒否し、通信失敗を成功扱いしない", async () => {
  const fetcher = vi.fn(async () => {
    throw new Error("connection lost");
  });
  vi.stubGlobal("fetch", fetcher);
  const input = {
    guild_id: "111",
    idempotency_key: "key",
    event_id: 1,
    expected_version: "v",
    changes: {},
  };
  for (const invalid of [
    { ...input, expected_version: undefined },
    { ...input, confirm: true },
    { ...input, user_id: "other" },
    { ...input, guild_id: 111 },
    { ...input, changes: { start_at: "2099-09-13T00:00:00" } },
  ]) {
    const response = await call("update_event", invalid);
    expect(response.error ?? response.result?.isError).toBeTruthy();
  }
  expect(fetcher).not.toHaveBeenCalled();
  const response = await call("update_event", input);
  expect(response.result.isError).toBe(true);
  expect(response.result.content[0].text).toContain("同じキー");
});

test("競合の理由を構造化データで返す", async () => {
  const error = {
    error: "conflict",
    message: "operation expired; this key cannot be reused",
  };
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json(error, { status: 409 })),
  );
  const response = await call("get_event_operation", {
    guild_id: "111",
    idempotency_key: "expired",
  });
  expect(response.result.isError).toBe(true);
  expect(response.result.structuredContent).toEqual({
    http_status: 409,
    ...error,
  });
});

test("文字数をRustと同じUnicode文字単位で検証し、キーはASCIIに限定する", async () => {
  const fetcher = vi.fn(async () =>
    Response.json({ database: "succeeded", discord: "not_required" }),
  );
  vi.stubGlobal("fetch", fetcher);
  const input = {
    guild_id: "111",
    idempotency_key: "key",
    changes: {
      name: "😀".repeat(32),
      description: "😀".repeat(1000),
      color: "#123456",
      start_at: "2099-09-13",
      end_at: "2099-09-13",
      is_all_day: true,
    },
  };
  expect((await call("create_event", input)).result.isError).toBeUndefined();
  fetcher.mockClear();
  for (const invalid of [
    { ...input, idempotency_key: "キー" },
    { ...input, changes: { ...input.changes, name: "😀".repeat(33) } },
  ]) {
    const response = await call("create_event", invalid);
    expect(response.error ?? response.result?.isError).toBeTruthy();
  }
  expect(fetcher).not.toHaveBeenCalled();
});
