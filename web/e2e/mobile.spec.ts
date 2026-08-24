import { expect, test } from "@playwright/test";
import { calendarToday, createEvent, dayCell, eventOn } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

// モバイル (タッチ操作) のカレンダー UX (#14)。
// スマホ相当のビューポート + タッチイベントで、タップがデスクトップのクリックと同じ入口
// (日付タップ → 作成ダイアログ、予定タップ → 概要ポップオーバー) につながることを確認する。
// タッチでは日付の select が長押し必須なので、タップは dateClick (タッチのときだけ有効) で拾っている。
// 長押しドラッグでの移動・リサイズは Playwright のタッチ API では再現できないため実機確認に委ねる

test.use({ viewport: { width: 375, height: 812 }, hasTouch: true });

// テスト間で同じ名前の予定が残らないよう、実行ごとに変える (events.spec.ts と同じ理由)
const stamp = Date.now().toString(36);
const title = `E2E モバイル ${stamp}`;

/** DatePicker のボタンに出る "yyyy/MM/dd" 部分 */
function formatSlash(date: Date): string {
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}/${m}/${d}`;
}

test("日付をタップすると、その日を開始日にした作成ダイアログが開く", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(page.getByRole("grid")).toBeVisible();

  const today = await calendarToday(page);
  await dayCell(page, today).tap();

  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await expect(dialog).toBeVisible();
  // 月表示のタップなので終日の予定になる
  await expect(dialog.getByLabel("開始日")).toContainText(formatSlash(today));
  // getByLabel だと Base UI の checkbox と隠しネイティブ input の両方に当たるので role で絞る
  await expect(dialog.getByRole("checkbox", { name: "終日" })).toBeChecked();
  await dialog.getByRole("button", { name: "キャンセル" }).tap();
  await expect(dialog).toBeHidden();
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

test("編集権限がないギルドでは日付をタップしても作成ダイアログが開かない", async ({
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
