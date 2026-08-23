import { expect, test } from "@playwright/test";
import { calendarToday, isoDate } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

// 「今日」のセルの hydration 不具合 (#48) の再現。
// FullCalendar が SSR 時のサーバー (TZ=UTC) の日付で描いた「今日」(aria-current="date") は
// React が属性の差分を patch しないため hydration 後も残り、ブラウザ (Asia/Tokyo) の今日とずれる。
// 実時刻では 0:00〜9:00 JST の間しかずれないので、ブラウザの Date だけを 48 時間進めて
// (サーバーの時計はそのまま) どの時刻に実行してもサーバーの「今日」と必ず別の日になるようにする

test("「今日」のセルがブラウザの今日を指す (SSR のサーバー日付が残らない)", async ({
  page,
}) => {
  await page.clock.setFixedTime(new Date(Date.now() + 48 * 60 * 60 * 1000));
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(page.getByRole("grid")).toBeVisible();

  const today = await calendarToday(page);
  const current = page.locator('[role="gridcell"][aria-current="date"]');
  await expect(current).toHaveCount(1);
  await expect(current).toHaveAttribute("data-date", isoDate(today));
});
