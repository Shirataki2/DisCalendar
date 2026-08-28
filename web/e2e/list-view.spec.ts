import { expect, test } from "@playwright/test";
import { createEvent, eventOn, openEventPopover } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

// 「リスト」ビュー (#92)。月 / 週 / 4日 / 日に加えたビュー切替で、予定が時系列に並ぶことと、
// ボタンが 1 つ増えてもスマートフォンの幅でツールバーがはみ出さないことを確認する

const guildId = E2E_GUILDS.admin.id;

// 他のテストと予定の名前がぶつからないよう、実行ごとに変える (events.spec.ts と同じ理由)
const stamp = Date.now().toString(36);
const title = `E2E リスト ${stamp}`;

/** ビュー切替 (ヘッダツールバー)。FullCalendar v7 は role=tablist / tab で出す */
const VIEW_TABS = ["月", "週", "4日", "日", "リスト"];

test("リストに切り替えると予定が並び、クリックで概要ポップオーバーが開く", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  // 既定の日時は今日なので、今月のリストに必ず入る
  await createEvent(page, title);

  await page.getByRole("tab", { name: "リスト", exact: true }).click();
  // 月グリッドから予定リストに入れ替わる
  await expect(page.getByRole("grid")).toHaveCount(0);
  // 日付ごとの listitem (日付の見出し + その日の予定)
  const day = page.getByRole("listitem").filter({ has: eventOn(page, title) });
  await expect(day).toBeVisible();

  // 日付の見出しは #63 に合わせて数字だけ ("8/28")。ja の既定は "2026年8月28日"
  const today = await page.evaluate(() => {
    const now = new Date();
    return `${now.getMonth() + 1}/${now.getDate()}`;
  });
  await expect(day.getByText(today, { exact: true })).toBeVisible();

  // 行をクリックすると月グリッドと同じ概要ポップオーバーが開く
  const popover = await openEventPopover(page, title);
  // 後片付けも兼ねて、そのまま削除する
  await popover.getByRole("button", { name: "削除" }).click();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "削除" })
    .click();
  await expect(eventOn(page, title)).toHaveCount(0);
  // 予定が無くなったら案内が出る (ja ロケール)
  await expect(page.getByText("表示する予定はありません")).toBeVisible();
});

test.describe("スマートフォンの幅", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true });

  test("ビュー切替のボタンが増えても横にはみ出さない", async ({ page }) => {
    await page.goto(`/dashboard/${guildId}`);
    await expect(page.getByRole("grid")).toBeVisible();

    // ボタンはすべて表示され、画面幅の内側に収まっている
    for (const name of VIEW_TABS) {
      const tab = page.getByRole("tab", { name, exact: true });
      await expect(tab).toBeVisible();
      const box = await tab.boundingBox();
      expect(box).not.toBeNull();
      if (!box) continue;
      expect(box.x).toBeGreaterThanOrEqual(0);
      expect(box.x + box.width).toBeLessThanOrEqual(375);
    }
    // ページ全体も横スクロールしない
    const overflow = await page.evaluate(
      () =>
        document.documentElement.scrollWidth -
        document.documentElement.clientWidth,
    );
    expect(overflow).toBeLessThanOrEqual(0);

    // タップでもリストに切り替えられる
    await page.getByRole("tab", { name: "リスト", exact: true }).tap();
    await expect(page.getByRole("grid")).toHaveCount(0);
  });
});
