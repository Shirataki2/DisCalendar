import { expect, test } from "@playwright/test";
import { Pool } from "pg";
import { openEventPopover } from "./calendar";
import { DATABASE_URL } from "./env";
import {
  E2E_BOT_ROLE_ID,
  E2E_EDITOR_ROLES,
  E2E_GUILDS,
  E2E_USER,
} from "./fixtures";

const createdEventPaths: string[] = [];
test.afterEach(async ({ request }) => {
  for (const path of createdEventPaths.splice(0)) {
    await request.delete(`/local/api/events/${path}`);
  }
});

test("メンション先を作成・編集・複製でき、日時変更でも保持される", async ({
  page,
}) => {
  const guildId = E2E_GUILDS.admin.id;
  await page.goto(`/dashboard/${guildId}`);
  await page.getByRole("button", { name: "新規作成", exact: true }).click();
  const dialog = page.getByRole("dialog");
  const title = `メンション ${Date.now()}`;
  await dialog.getByLabel("タイトル").fill(title);
  await dialog
    .getByRole("checkbox", { name: "@everyone（全員）", exact: true })
    .check();
  await dialog.getByRole("combobox", { name: "メンションするロール" }).click();
  await page
    .getByRole("option", { name: E2E_EDITOR_ROLES[0].name, exact: true })
    .click();
  await dialog
    .getByLabel("メンションするユーザーID", { exact: true })
    .fill(E2E_USER.discordId);
  await dialog
    .getByRole("button", { name: "ユーザーを追加", exact: true })
    .click();
  await expect(
    dialog.getByRole("button", { name: `@${E2E_USER.name}のメンションを削除` }),
  ).toBeVisible();
  // 事前通知が空でも開始時刻のメンション先は独立して保存される。
  while (
    await dialog
      .getByRole("button", { name: "この通知を削除", exact: true })
      .count()
  ) {
    await dialog
      .getByRole("button", { name: "この通知を削除", exact: true })
      .first()
      .click();
  }
  const created = page.waitForResponse(
    (res) =>
      res.url().endsWith(`/events/${guildId}`) &&
      res.request().method() === "POST",
  );
  await dialog.getByRole("button", { name: "作成", exact: true }).click();
  const response = await created;
  expect(response.status()).toBe(201);
  const event = await response.json();
  createdEventPaths.push(`${guildId}/${event.id}`);
  const mentions = [
    { type: "everyone" },
    { type: "role", id: E2E_EDITOR_ROLES[0].id },
    { type: "user", id: E2E_USER.discordId },
  ];
  expect(event.notification_mentions).toEqual(mentions);
  const popover = await openEventPopover(page, title);
  await popover.getByRole("button", { name: "編集", exact: true }).click();
  await expect(
    dialog.getByRole("checkbox", { name: "@everyone（全員）", exact: true }),
  ).toBeChecked();
  await expect(
    dialog.getByRole("button", {
      name: `@${E2E_EDITOR_ROLES[0].name}のメンションを削除`,
    }),
  ).toBeVisible();
  await dialog
    .getByLabel("説明", { exact: true })
    .fill("メンション先を保持して編集");
  await dialog.getByLabel("終了時刻", { exact: true }).fill("23:45");
  const updated = page.waitForResponse(
    (res) =>
      res.url().endsWith(`/events/${guildId}/${event.id}`) &&
      res.request().method() === "PUT",
  );
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  expect((await (await updated).json()).notification_mentions).toEqual(
    mentions,
  );
  const reopened = await openEventPopover(page, title);
  await reopened.getByRole("button", { name: "複製", exact: true }).click();
  await expect(
    dialog.getByRole("checkbox", { name: "@everyone（全員）", exact: true }),
  ).toBeChecked();
  await dialog.getByLabel("タイトル").fill(`${title} 複製`);
  const duplicated = page.waitForResponse(
    (res) =>
      res.url().endsWith(`/events/${guildId}`) &&
      res.request().method() === "POST",
  );
  await dialog.getByRole("button", { name: "作成", exact: true }).click();
  const copy = await (await duplicated).json();
  createdEventPaths.push(`${guildId}/${copy.id}`);
  expect(copy.notification_mentions).toEqual(mentions);
  const edit = await openEventPopover(page, title);
  await edit.getByRole("button", { name: "編集", exact: true }).click();
  await dialog
    .getByRole("checkbox", { name: "@everyone（全員）", exact: true })
    .uncheck();
  await dialog
    .getByRole("button", {
      name: `@${E2E_EDITOR_ROLES[0].name}のメンションを削除`,
    })
    .click();
  await dialog
    .getByRole("button", { name: `@${E2E_USER.name}のメンションを削除` })
    .click();
  const cleared = page.waitForResponse(
    (res) =>
      res.url().endsWith(`/events/${guildId}/${event.id}`) &&
      res.request().method() === "PUT",
  );
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  expect((await (await cleared).json()).notification_mentions).toEqual([]);
});

test("API 直呼びでも権限・所属・ID・上限を検証する", async ({ page }) => {
  await page.goto(`/dashboard/${E2E_GUILDS.noUserEventsPerm.id}`);
  const input = {
    name: "メンション検証",
    color: "#2196F3",
    start_at: "2026-09-12T10:00:00",
    end_at: "2026-09-12T11:00:00",
  };
  for (const mention of [
    { type: "everyone" },
    { type: "role", id: E2E_EDITOR_ROLES[0].id },
  ]) {
    const result = await page.request.post(
      `/local/api/events/${E2E_GUILDS.noUserEventsPerm.id}`,
      { data: { ...input, notification_mentions: [mention] } },
    );
    expect(result.status()).toBe(403);
    expect((await result.json()).message).toContain("メンション");
  }
  const personal = await page.request.post(
    `/local/api/events/${E2E_GUILDS.noUserEventsPerm.id}`,
    {
      data: {
        ...input,
        notification_mentions: [{ type: "user", id: E2E_USER.discordId }],
      },
    },
  );
  expect(personal.status()).toBe(201);
  const rolesUrl = `/local/api/guilds/${E2E_GUILDS.noUserEventsPerm.id}/roles`;
  const editorRoles = await (await page.request.get(rolesUrl)).json();
  const mentionRoles = await (
    await page.request.get(`${rolesUrl}?for_mentions=true`)
  ).json();
  expect(
    editorRoles.some((role: { id: string }) => role.id === E2E_BOT_ROLE_ID),
  ).toBe(false);
  expect(
    mentionRoles.some((role: { id: string }) => role.id === E2E_BOT_ROLE_ID),
  ).toBe(true);
  const managed = await page.request.post(
    `/local/api/events/${E2E_GUILDS.noUserEventsPerm.id}`,
    {
      data: {
        ...input,
        notification_mentions: [{ type: "role", id: E2E_BOT_ROLE_ID }],
      },
    },
  );
  expect(managed.status()).toBe(201);
  createdEventPaths.push(
    `${E2E_GUILDS.noUserEventsPerm.id}/${(await managed.json()).id}`,
  );
  const personalEvent = await personal.json();
  const personalPath = `${E2E_GUILDS.noUserEventsPerm.id}/${personalEvent.id}`;
  createdEventPaths.push(personalPath);
  const pool = new Pool({ connectionString: DATABASE_URL });
  try {
    for (const mention of [
      { type: "everyone" },
      { type: "role", id: E2E_EDITOR_ROLES[0].id },
    ]) {
      // 別の管理者が保存した一斉メンションを、省略した更新で再利用させない。
      await pool.query(
        "UPDATE events SET notification_mentions=$1 WHERE id=$2",
        [JSON.stringify([mention]), personalEvent.id],
      );
      const update = await page.request.put(
        `/local/api/events/${personalPath}`,
        {
          data: { ...input, notifications: [{ num: 30, unit: "minutes" }] },
        },
      );
      expect(update.status()).toBe(403);
      expect((await update.json()).message).toContain("メンション");
      const clear = await page.request.put(
        `/local/api/events/${personalPath}`,
        {
          data: { ...input, notification_mentions: [] },
        },
      );
      expect(clear.status()).toBe(200);
      expect((await clear.json()).notification_mentions).toEqual([]);
    }
  } finally {
    await pool.end();
  }
  for (const mentions of [
    [{ type: "user", id: "999999999999999999" }],
    [{ type: "role", id: "999999999999999999" }],
    [{ type: "user", id: "0" }],
    [{ type: "user", id: 123 }],
    [{ type: "everyone" }, { type: "everyone" }],
    Array.from({ length: 11 }, () => ({ type: "everyone" })),
  ]) {
    const result = await page.request.post(
      `/local/api/events/${E2E_GUILDS.admin.id}`,
      { data: { ...input, notification_mentions: mentions } },
    );
    expect(result.status()).toBe(400);
  }
  const restricted = await page.request.get(
    `/local/api/guilds/${E2E_GUILDS.member.id}/mention-members?ids=${E2E_USER.discordId}`,
  );
  expect(restricted.status()).toBe(403);
});

test("任意のメンバーIDを連続照会すると429で制限する", async ({ request }) => {
  let status = 200;
  for (let batch = 0; batch < 15 && status !== 429; batch++) {
    // Snowflake を number にしないよう、一意な文字列から作る。
    const query = Array.from(
      { length: 10 },
      (_, i) => `90000000000000${String(batch).padStart(2, "0")}${i}`,
    ).join(",");
    const response = await request.get(
      `/local/api/guilds/${E2E_GUILDS.admin.id}/mention-members?ids=${query}`,
    );
    status = response.status();
    expect([200, 429]).toContain(status);
  }
  expect(status).toBe(429);
});
