import { expect, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

// 日本の祝日の表示 (#97)。祝日は年ごとに動くので、ブラウザの時計を固定して
// 祝日が確定している 2026 年 1 月 (元日 = 木曜) を表示する。サーバーの時計はそのままだが、
// カレンダーはマウント後にブラウザの日付で描画される (#48) ため表示月はこれで決まる

test.beforeEach(async ({ page }) => {
  await page.clock.setFixedTime(new Date(2026, 0, 15, 12, 0, 0));
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(page.getByRole("grid")).toBeVisible();
});

test("月ビューの祝日セルに祝日名が表示され、赤くなる", async ({ page }) => {
  const newYear = page.locator('[role="gridcell"][data-date="2026-01-01"]');
  await expect(newYear).toContainText("元日");
  await expect(newYear.locator(".cal-day-red")).toBeVisible();
});

test("月ビューの日曜は赤、土曜は青、平日は色なし", async ({ page }) => {
  await expect(
    page.locator('[role="gridcell"][data-date="2026-01-04"] .cal-day-red'),
  ).toBeVisible();
  await expect(
    page.locator('[role="gridcell"][data-date="2026-01-03"] .cal-day-blue'),
  ).toBeVisible();
  await expect(
    page.locator(
      '[role="gridcell"][data-date="2026-01-02"] :is(.cal-day-red, .cal-day-blue)',
    ),
  ).toHaveCount(0);
});

test("月ビューの曜日ヘッダは曜日だけで塗られる (祝日を含む列でも赤くならない)", async ({
  page,
}) => {
  // 先頭週に元日 (木) があるが、木曜のヘッダは赤くならない
  await expect(
    page.getByRole("columnheader", { name: "木曜日" }),
  ).not.toHaveClass(/cal-day-red/);
  await expect(page.getByRole("columnheader", { name: "日曜日" })).toHaveClass(
    /cal-day-red/,
  );
  await expect(page.getByRole("columnheader", { name: "土曜日" })).toHaveClass(
    /cal-day-blue/,
  );
});

test("週ビューの日付ヘッダは祝日も赤くなる", async ({ page }) => {
  // 1/15 (木) の週 (1/11 〜 1/17) に成人の日 (1/12 月) がある
  await page.getByRole("tab", { name: "週", exact: true }).click();
  await expect(
    page.getByRole("columnheader").filter({ hasText: "12(月)" }),
  ).toHaveClass(/cal-day-red/);
  await expect(
    page.getByRole("columnheader").filter({ hasText: "13(火)" }),
  ).not.toHaveClass(/cal-day-(red|blue)/);
});
