import { expect, test } from "vitest";
import { consentSelection } from "./config";

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
