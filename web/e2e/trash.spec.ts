import { expect, type Page, test } from "@playwright/test";
import {
  calendarToday,
  createEvent,
  dayCell,
  eventOn,
  isoDate,
} from "./calendar";
import { E2E_GUILDS, E2E_USER } from "./fixtures";

// 削除した予定 (ゴミ箱、#159)。サーバー設定の一覧から元に戻す・完全に削除する。
// admin ギルドではテストユーザーがオーナー、member ギルドでは権限のない一般メンバー (Discord モックの定義)

const guildId = E2E_GUILDS.admin.id;
const stamp = Date.now().toString(36);

async function openSettings(page: Page) {
  await page.getByRole("button", { name: "サーバー設定" }).click();
  const dialog = page.getByRole("dialog", { name: "サーバー設定" });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** API で予定を作ってすぐ消す (ブラウザと同じ cookie で叩く)。ゴミ箱に入った予定の ID を返す */
async function createTrashedEvent(page: Page, name: string, date: string) {
  const created = await page.request.post(`/local/api/events/${guildId}`, {
    data: {
      name,
      description: null,
      notifications: [],
      color: "#2196F3",
      is_all_day: false,
      start_at: `${date}T10:00:00`,
      end_at: `${date}T11:00:00`,
    },
  });
  expect(created.status()).toBe(201);
  const { id } = await created.json();
  const deleted = await page.request.delete(
    `/local/api/events/${guildId}/${id}`,
  );
  expect(deleted.status()).toBe(204);
  return id as number;
}

test.describe("管理権限のあるギルド", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`/dashboard/${guildId}`);
    await expect(page.getByRole("grid")).toBeVisible();
  });

  test("サーバー設定の一覧から元に戻すと、カレンダーに戻る", async ({
    page,
  }) => {
    const title = `E2E ゴミ箱 復元 ${stamp}`;
    const today = await calendarToday(page);
    const id = await createTrashedEvent(page, title, isoDate(today));
    await page.reload();
    await expect(page.getByRole("grid")).toBeVisible();
    await expect(eventOn(page, title)).toHaveCount(0);

    const dialog = await openSettings(page);
    const section = dialog.getByRole("region", { name: "削除した予定" });
    const item = section.getByRole("listitem").filter({ hasText: title });
    // 誰がいつ消したかを併記する
    await expect(item).toContainText(`削除: ${E2E_USER.name}`);
    await expect(item).toContainText("まで元に戻せます");

    const restored = page.waitForResponse(
      (res) =>
        res.url().endsWith(`/${id}/restore`) &&
        res.request().method() === "POST",
    );
    await item.getByRole("button", { name: `「${title}」を元に戻す` }).click();
    expect((await restored).status()).toBe(200);
    await expect(section.getByRole("status")).toContainText(
      `「${title}」を元に戻しました。`,
    );
    await expect(item).toHaveCount(0);

    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(eventOn(dayCell(page, today), title)).toBeVisible();

    const cleanup = await page.request.delete(
      `/local/api/events/${guildId}/${id}`,
    );
    expect(cleanup.status()).toBe(204);
  });

  test("完全に削除すると一覧から消え、元に戻せなくなる", async ({ page }) => {
    const title = `E2E ゴミ箱 完全削除 ${stamp}`;
    const today = await calendarToday(page);
    const id = await createTrashedEvent(page, title, isoDate(today));

    const dialog = await openSettings(page);
    const section = dialog.getByRole("region", { name: "削除した予定" });
    const item = section.getByRole("listitem").filter({ hasText: title });
    await item
      .getByRole("button", { name: `「${title}」を完全に削除` })
      .click();
    const confirm = page.getByRole("alertdialog");
    await expect(confirm).toContainText("完全に削除しますか？");
    await expect(confirm).toContainText(title);
    const purged = page.waitForResponse(
      (res) =>
        res.url().endsWith(`/${id}/purge`) &&
        res.request().method() === "DELETE",
    );
    await confirm.getByRole("button", { name: "完全に削除" }).click();
    expect((await purged).status()).toBe(204);
    await expect(section.getByRole("status")).toContainText(
      `「${title}」を完全に削除しました。`,
    );
    await expect(item).toHaveCount(0);

    const restore = await page.request.post(
      `/local/api/events/${guildId}/${id}/restore`,
    );
    expect(restore.status()).toBe(404);
  });

  test("削除した予定は一覧・共有ページに出ず、元に戻すと同じ共有 URL で見られる", async ({
    page,
  }) => {
    const title = `E2E ゴミ箱 共有 ${stamp}`;
    const today = await calendarToday(page);
    await createEvent(page, title);
    const list = await page.request.get(
      `/local/api/events/${guildId}?start=${isoDate(today)}T00:00:00&end=${isoDate(today)}T23:59:59`,
    );
    const event = (await list.json()).find(
      (item: { name: string }) => item.name === title,
    );
    const share = await page.request.post(
      `/local/api/events/${guildId}/${event.id}/share`,
    );
    const { token } = await share.json();
    expect((await page.request.get(`/local/api/share/${token}`)).status()).toBe(
      200,
    );

    const deleted = await page.request.delete(
      `/local/api/events/${guildId}/${event.id}`,
    );
    expect(deleted.status()).toBe(204);
    // 削除中は共有ページも履歴も見えず、編集もできない
    expect((await page.request.get(`/local/api/share/${token}`)).status()).toBe(
      404,
    );
    expect(
      (
        await page.request.get(
          `/local/api/events/${guildId}/${event.id}/history`,
        )
      ).status(),
    ).toBe(404);
    // 2 回消しても 404 (削除した人・日時を上書きしない)
    expect(
      (
        await page.request.delete(`/local/api/events/${guildId}/${event.id}`)
      ).status(),
    ).toBe(404);

    const restored = await page.request.post(
      `/local/api/events/${guildId}/${event.id}/restore`,
    );
    expect(restored.status()).toBe(200);
    expect(await restored.json()).toMatchObject({
      id: event.id,
      name: title,
      start_at: event.start_at,
      discord_scheduled_event_id: null,
    });
    expect((await page.request.get(`/local/api/share/${token}`)).status()).toBe(
      200,
    );

    const cleanup = await page.request.delete(
      `/local/api/events/${guildId}/${event.id}`,
    );
    expect(cleanup.status()).toBe(204);
  });
});

test.describe("管理権限のないギルド", () => {
  test("削除した予定の一覧は出ず、API でも一覧と完全削除が拒否される (表示だけの制御ではない)", async ({
    page,
  }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.member.id}`);
    await expect(page.getByText(E2E_GUILDS.member.name)).toBeVisible();
    const dialog = await openSettings(page);
    await expect(
      dialog.getByRole("heading", { name: "通知", exact: false }).first(),
    ).toBeVisible();
    await expect(
      dialog.getByRole("heading", { name: "削除した予定" }),
    ).toHaveCount(0);

    const trash = await page.request.get(
      `/local/api/events/${E2E_GUILDS.member.id}/trash`,
    );
    expect(trash.status()).toBe(403);
    const purge = await page.request.delete(
      `/local/api/events/${E2E_GUILDS.member.id}/1/purge`,
    );
    expect(purge.status()).toBe(403);
    // restricted モードで編集できない人は、元に戻すこともできない
    const restore = await page.request.post(
      `/local/api/events/${E2E_GUILDS.member.id}/1/restore`,
    );
    expect(restore.status()).toBe(403);
  });
});
