import { afterEach, expect, test, vi } from "vitest";
import {
  consentSelection,
  mcpConnectionsEnabled,
  mcpRequestAllowed,
} from "./config";

const guild = "123456789012345678";
test("同意は要求scopeと現在利用可能なサーバーに限定する", () => {
  expect(
    consentSelection(
      ["events:read"],
      [guild],
      ["events:read", "events:delete"],
      [guild],
    ),
  ).toEqual({ scopes: ["events:read"], guildIds: [guild] });
  for (const [scopes, ids] of [
    [[], [guild]],
    [["unknown"], [guild]],
    [["events:delete"], [guild]],
    [["events:read"], []],
    [["events:read"], ["999"]],
    [["events:read"], [guild, guild]],
    [["events:read", "events:read"], [guild]],
  ])
    expect(() =>
      consentSelection(scopes, ids, ["events:read"], [guild]),
    ).toThrow();
});

afterEach(() => vi.unstubAllEnvs());

test("新規接続停止は全停止に従い、明示したfalseだけで新規を止める", () => {
  vi.stubEnv("MCP_ENABLED", "false");
  expect(mcpConnectionsEnabled()).toBe(false);
  vi.stubEnv("MCP_ENABLED", "true");
  expect(mcpConnectionsEnabled()).toBe(true);
  vi.stubEnv("MCP_NEW_CONNECTIONS_ENABLED", "false");
  expect(mcpConnectionsEnabled()).toBe(false);
});

test("MCPは固定Hostと同一Originだけを許可し、非ブラウザのOrigin省略を許可する", () => {
  vi.stubEnv("BETTER_AUTH_URL", "https://calendar.example");
  for (const [host, origin, allowed] of [
    ["calendar.example", undefined, true],
    ["calendar.example", "https://calendar.example", true],
    ["calendar.example", "null", false],
    ["calendar.example", "", false],
    ["calendar.example", "http://calendar.example", false],
    ["calendar.example", "https://other.example", false],
    ["calendar.example:444", undefined, false],
    ["other.example", undefined, false],
    [undefined, undefined, false],
  ] as const) {
    const headers = new Headers({ "x-forwarded-host": "calendar.example" });
    if (host !== undefined) headers.set("host", host);
    if (origin !== undefined) headers.set("origin", origin);
    expect(
      mcpRequestAllowed(
        new Request("https://calendar.example/mcp", { headers }),
      ),
    ).toBe(allowed);
  }
});
