import { describe, expect, test } from "vitest";
import type { GuildApiToken, GuildWebhook } from "@/lib/api/types";
import { groupIntegrationsByCreator } from "./integrations-by-creator";

const webhook = (
  id: string,
  createdBy: string,
  createdAt: string,
): GuildWebhook => ({
  id,
  url: `https://example.com/${id}`,
  kind: "json",
  enabled: true,
  consecutive_failures: 0,
  disabled_reason: null,
  created_by: createdBy,
  created_at: createdAt,
  deliveries: [],
});

const token = (
  id: string,
  createdBy: string,
  createdAt: string,
): GuildApiToken => ({
  id,
  name: `token ${id}`,
  token_prefix: `dct_${id}`,
  scope: "create",
  expires_at: null,
  last_used_at: null,
  revoked_at: null,
  created_by: createdBy,
  created_at: createdAt,
  logs: [],
});

describe("groupIntegrationsByCreator", () => {
  test("何も無ければ空", () => {
    expect(groupIntegrationsByCreator([], [])).toEqual([]);
  });

  test("Webhook と API トークンを登録したユーザーごとにまとめ、数の多い順・同数なら先に登録した順に並べる", () => {
    // Snowflake は Number の精度を超えるので、文字列のまま比較できることも確かめる
    const alice = "123456789012345678";
    const bob = "123456789012345679";
    const carol = "987654321098765432";
    const groups = groupIntegrationsByCreator(
      [
        webhook("1", bob, "2026-11-02T00:00:00Z"),
        webhook("2", alice, "2026-11-03T00:00:00Z"),
        webhook("3", carol, "2026-11-04T00:00:00Z"),
      ],
      [
        token("10", alice, "2026-11-05T00:00:00Z"),
        token("11", carol, "2026-11-01T00:00:00Z"),
      ],
    );
    expect(groups.map((group) => group.userId)).toEqual([carol, alice, bob]);
    expect(groups[0].webhooks.map((hook) => hook.id)).toEqual(["3"]);
    expect(groups[0].tokens.map((item) => item.id)).toEqual(["11"]);
    expect(groups[1].webhooks.map((hook) => hook.id)).toEqual(["2"]);
    expect(groups[1].tokens.map((item) => item.id)).toEqual(["10"]);
    expect(groups[2]).toEqual({
      userId: bob,
      webhooks: [webhook("1", bob, "2026-11-02T00:00:00Z")],
      tokens: [],
    });
  });
});
