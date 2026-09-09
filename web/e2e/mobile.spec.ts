import { expect, test } from "@playwright/test";
import {
  calendarToday,
  createEvent,
  dayCell,
  eventOn,
  neighborDay,
} from "./calendar";
import { E2E_GUILDS } from "./fixtures";

// モバイル (タッチ操作) のカレンダー UX (#14)。
// スマホ相当のビューポート + タッチイベントで、タップがデスクトップのクリックと同じ入口
// (日付タップ → クイック追加、予定タップ → 概要ポップオーバー) につながることを確認する。
// タッチでは日付の select が長押し必須なので、タップは dateClick (タッチのときだけ有効) で拾っている。
// 長押しドラッグでの移動・リサイズは Playwright のタッチ API では再現できないため実機確認に委ねる

test.use({ viewport: { width: 375, height: 812 }, hasTouch: true });

// テスト間で同じ名前の予定が残らないよう、実行ごとに変える (events.spec.ts と同じ理由)
const stamp = Date.now().toString(36);
const title = `E2E モバイル ${stamp}`;

/** クイック追加の期間に出る "yyyy/MM/dd" */
function formatSlash(date: Date): string {
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}/${m}/${d}`;
}

test("日付をタップすると、その日のクイック追加が開く", async ({ page }) => {
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(page.getByRole("grid")).toBeVisible();

  const today = await calendarToday(page);
  await dayCell(page, today).tap();

  const quickAdd = page.getByRole("dialog", { name: "予定をクイック追加" });
  await expect(quickAdd).toBeVisible();
  await expect(quickAdd).toContainText(formatSlash(today));
  await expect(quickAdd.getByLabel("タイトル")).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(quickAdd).toBeHidden();

  await dayCell(page, today).tap();
  await quickAdd.getByLabel("タイトル").fill("破棄するモバイル予定");
  const anotherDay = await neighborDay(page, today);
  const outside = await dayCell(page, anotherDay).boundingBox();
  if (!outside) throw new Error("タップ先の日付が表示されていません");
  await page.touchscreen.tap(outside.x + 12, outside.y + 45);
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
  await expect(quickAdd).toBeHidden();
  await expect(eventOn(page, "破棄するモバイル予定")).toHaveCount(0);

  await dayCell(page, anotherDay).tap();
  await expect(quickAdd).toContainText(formatSlash(anotherDay));
});

test("予定をタップすると概要ポップオーバーが開き、タッチで削除まで行える", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(page.getByRole("grid")).toBeVisible();
  await createEvent(page, title);

  await eventOn(page, title).tap();
  const popover = page.getByRole("dialog").filter({ hasText: title });
  await expect(popover).toBeVisible();
  await expect(popover.getByRole("button", { name: "編集" })).toBeVisible();

  // 後片付けも兼ねて、そのままタッチで削除する
  await popover.getByRole("button", { name: "削除" }).tap();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "削除" })
    .tap();
  await expect(eventOn(page, title)).toHaveCount(0);
});

test("編集権限がないギルドでは日付をタップしてもクイック追加が開かない", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.member.id}`);
  await expect(page.getByRole("grid")).toBeVisible();

  const today = await calendarToday(page);
  await dayCell(page, today).tap();
  // 開くとしたら少し遅れて開くので、待ってから居ないことを確認する
  await page.waitForTimeout(500);
  await expect(page.getByRole("dialog")).toHaveCount(0);
});
