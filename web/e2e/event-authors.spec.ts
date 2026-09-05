import { expect, test } from "@playwright/test";
import { createEvent, openEventPopover } from "./calendar";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS, E2E_USER } from "./fixtures";
import { deleteEventsNamed } from "./seed";

const guildId = E2E_GUILDS.admin.id;
const title = `作成者確認 ${Date.now().toString(36)}`;
test.afterAll(async () => {
  await deleteEventsNamed(DATABASE_URL, [title]);
});

test("メンバー情報は認証と所属確認を通し、ID の形式・上限を検証する", async ({
  request,
  playwright,
}) => {
  const path = `/local/api/guilds/${guildId}/members?ids=${E2E_USER.discordId}`;
  const response = await request.get(path);
  expect(response.status()).toBe(200);
  expect(await response.json()).toEqual([
    {
      user_id: E2E_USER.discordId,
      display_name: E2E_USER.name,
      avatar_url: null,
    },
  ]);
  const guest = await playwright.request.newContext({
    baseURL: test.info().project.use.baseURL,
    storageState: { cookies: [], origins: [] },
  });
  expect((await guest.get(path)).status()).toBe(401);
  await guest.dispose();
  expect(
    (
      await request.get(
        `/local/api/guilds/999999999999999999/members?ids=${E2E_USER.discordId}`,
      )
    ).status(),
  ).toBe(403);
  for (const ids of ["abc", "1/2", Array(21).fill("1").join(",")]) {
    expect(
      (
        await request.get(`/local/api/guilds/${guildId}/members`, {
          params: { ids },
        })
      ).status(),
    ).toBe(400);
  }
  const departed = await request.get(
    `/local/api/guilds/${guildId}/members?ids=999999999999999999`,
  );
  expect(await departed.json()).toEqual([
    { user_id: "999999999999999999", display_name: null, avatar_url: null },
  ]);
});

test("一覧では解決せず、退出済みと取得失敗を区別して表示する", async ({
  page,
}) => {
  let lookups = 0;
  await page.route("**/local/api/guilds/*/members?*", async (route) => {
    lookups++;
    await route.fulfill({
      json: [
        { user_id: E2E_USER.discordId, display_name: null, avatar_url: null },
      ],
    });
  });
  await page.goto(`/dashboard/${guildId}`);
  await createEvent(page, title);
  expect(lookups).toBe(0);
  const popover = await openEventPopover(page, title);
  await expect(popover).toContainText("退出したメンバー");
  // 開発時は Strict Mode の再マウントで中断・再取得されることがある。
  expect(lookups).toBeGreaterThan(0);
  await page.unroute("**/local/api/guilds/*/members?*");
  await page.route("**/local/api/guilds/*/members?*", (route) =>
    route.fulfill({
      status: 503,
      json: { error: "unavailable", message: "test" },
    }),
  );
  await page.reload();
  const reopened = await openEventPopover(page, title);
  await expect(reopened).toContainText("メンバー情報を取得できません");
  await expect(reopened).not.toContainText("退出したメンバー");
});

test("操作者を本文で偽装できず、更新しても作成者は変わらない", async ({
  request,
}) => {
  const body = {
    name: "操作者の検証",
    notifications: [],
    color: "#2196F3",
    is_all_day: false,
    start_at: "2026-01-01T10:00:00",
    end_at: "2026-01-01T11:00:00",
    created_by: "999",
    updated_by: "999",
    updated_at: "2000-01-01T00:00:00",
  };
  const created = await request.post(`/local/api/events/${guildId}`, {
    data: body,
  });
  expect(created.status()).toBe(201);
  const event = await created.json();
  try {
    expect(event.created_by).toBe(E2E_USER.discordId);
    expect(event.updated_by).toBeNull();
    expect(event.updated_at).toBeNull();
    const updated = await request.put(
      `/local/api/events/${guildId}/${event.id}`,
      { data: body },
    );
    expect(updated.status()).toBe(200);
    const result = await updated.json();
    expect(result.created_by).toBe(E2E_USER.discordId);
    expect(result.updated_by).toBe(E2E_USER.discordId);
    expect(result.updated_at).not.toBe(body.updated_at);
    expect(result.updated_at).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:/);
  } finally {
    await request.delete(`/local/api/events/${guildId}/${event.id}`);
  }
});
