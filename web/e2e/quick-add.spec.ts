import { expect, test } from "@playwright/test";
import {
  calendarToday,
  dayCell,
  eventOn,
  isoDate,
  neighborDay,
} from "./calendar";
import { E2E_GUILDS } from "./fixtures";

const guildId = E2E_GUILDS.admin.id;
const eventsApi = new RegExp(`/local/api/events/${guildId}(/|\\?|$)`);
const createdEventIds: number[] = [];

test.beforeEach(async ({ page }) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
});

test.afterEach(async ({ request }) => {
  for (const id of createdEventIds.splice(0)) {
    await request.delete(`/local/api/events/${guildId}/${id}`);
  }
});

test("月の日付をクリックしてタイトルを Enter で作成できる", async ({
  page,
}) => {
  const today = await calendarToday(page);
  const title = `E2E クイック追加 ${Date.now()}`;
  await dayCell(page, today).click({ position: { x: 12, y: 45 } });

  const quickAdd = page.getByRole("dialog", { name: "予定をクイック追加" });
  const input = quickAdd.getByLabel("タイトル");
  await expect(quickAdd).toContainText(isoDate(today).replaceAll("-", "/"));
  await expect(input).toBeFocused();
  await input.fill(title);
  await expect(eventOn(dayCell(page, today), title)).toBeVisible();

  const created = page.waitForResponse(
    (response) =>
      eventsApi.test(response.url()) && response.request().method() === "POST",
  );
  await input.press("Enter");
  const response = await created;
  expect(response.status()).toBe(201);
  createdEventIds.push((await response.json()).id);
  expect(response.request().postDataJSON()).toMatchObject({
    name: title,
    description: null,
    color: "#F44336",
    notifications: [
      { num: 1, unit: "days" },
      { num: 1, unit: "hours" },
    ],
    discord_scheduled_event: false,
  });
  await expect(quickAdd).toBeHidden();
  await expect(eventOn(dayCell(page, today), title)).toBeVisible();
});

test("空のタイトルを検証し、Esc と外部クリックで取り消せる", async ({
  page,
}) => {
  const today = await calendarToday(page);
  const quickAdd = page.getByRole("dialog", { name: "予定をクイック追加" });

  await dayCell(page, today).click({ position: { x: 12, y: 45 } });
  await quickAdd.getByLabel("タイトル").fill("   ");
  await quickAdd.getByLabel("タイトル").press("Enter");
  await expect(quickAdd.getByRole("alert")).toHaveText(
    "タイトルを入力してください",
  );
  await quickAdd.getByLabel("タイトル").fill("あ".repeat(33));
  await quickAdd.getByLabel("タイトル").press("Enter");
  await expect(quickAdd.getByRole("alert")).toHaveText(
    "タイトルは32文字以内で入力してください",
  );
  await page.keyboard.press("Escape");
  await expect(quickAdd).toBeHidden();

  await page.getByRole("tab", { name: "日", exact: true }).click();
  const startSlot = page.locator('[data-time="10:00:00"]').last();
  await startSlot.scrollIntoViewIfNeeded();
  const start = await startSlot.boundingBox();
  const end = await page.locator('[data-time="10:30:00"]').last().boundingBox();
  const outside = await page
    .locator('[data-time="11:00:00"]')
    .last()
    .boundingBox();
  if (!start || !end || !outside)
    throw new Error("選択する時間枠が表示されていません");

  const x = start.x + start.width / 2;
  await page.mouse.move(x, start.y + start.height / 4);
  await page.mouse.down();
  await page.mouse.move(x, end.y + end.height / 4, { steps: 15 });
  await page.mouse.up();
  await quickAdd.getByLabel("タイトル").fill("破棄するタイトル");
  await expect(eventOn(page, "破棄するタイトル")).toBeVisible();
  const outsideX = outside.x + outside.width - 40;
  await page.mouse.click(outsideX, outside.y + outside.height / 4);
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
  await expect(eventOn(page, "破棄するタイトル")).toHaveCount(0);
  await expect(quickAdd).toBeHidden();

  await page.mouse.click(outsideX, outside.y + outside.height / 4);
  await expect(quickAdd).toBeVisible();
});

test("詳細入力へタイトルと日時を引き継ぐ", async ({ page }) => {
  const today = await calendarToday(page);
  const target = await neighborDay(page, today);
  await dayCell(page, target).click({ position: { x: 12, y: 45 } });
  const quickAdd = page.getByRole("dialog", { name: "予定をクイック追加" });
  await quickAdd.getByLabel("タイトル").fill("詳細へ引き継ぐ予定");
  await quickAdd.getByRole("button", { name: "詳細を入力" }).click();

  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await expect(dialog.getByLabel("タイトル")).toHaveValue("詳細へ引き継ぐ予定");
  await expect(dialog.getByLabel("開始日")).toContainText(
    isoDate(target).replaceAll("-", "/"),
  );
});

test("作成に失敗したらエラーを表示して再試行できる", async ({ page }) => {
  let attempts = 0;
  await page.route(eventsApi, async (route) => {
    if (route.request().method() !== "POST" || attempts++ > 0)
      return route.continue();
    await route.fulfill({
      status: 500,
      contentType: "application/json",
      body: JSON.stringify({ error: "internal_error", message: "e2e" }),
    });
  });
  const today = await calendarToday(page);
  const title = `E2E クイック再試行 ${Date.now()}`;
  await dayCell(page, today).click({ position: { x: 12, y: 45 } });
  const quickAdd = page.getByRole("dialog", { name: "予定をクイック追加" });
  const input = quickAdd.getByLabel("タイトル");
  await input.fill(title);
  await input.press("Enter");
  await expect(quickAdd.getByRole("alert")).toHaveText(
    "サーバーでエラーが発生しました (500)",
  );
  await expect(input).toBeEnabled();

  const created = page.waitForResponse(
    (response) =>
      eventsApi.test(response.url()) &&
      response.request().method() === "POST" &&
      response.status() === 201,
  );
  await input.press("Enter");
  const response = await created;
  createdEventIds.push((await response.json()).id);
  await expect(eventOn(page, title)).toBeVisible();
});
