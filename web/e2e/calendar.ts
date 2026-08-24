import { expect, type Locator, type Page } from "@playwright/test";

// FullCalendar (月表示) を操作するヘルパー。
// v7 はクラス名がハッシュ化されているので、ARIA ロールと data-date 属性で要素を探す
// (日付セル: role=gridcell + data-date="YYYY-MM-DD"、予定: その中の role=button "H:mm タイトル")

export function isoDate(date: Date): string {
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

export function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

/** 月表示の日付セル */
export function dayCell(page: Page, date: Date): Locator {
  return page.locator(`[role="gridcell"][data-date="${isoDate(date)}"]`);
}

/**
 * ブラウザから見た「今日」(年月日だけのローカル Date)。
 * テストを動かす Node (CI は UTC) とブラウザ (Asia/Tokyo) で日付が違うことがあるので、new Date() ではなく
 * ブラウザの Date から取る (「新規作成」の既定日時もブラウザの Date で決まる)。
 * FullCalendar の aria-current="date" は SSR 時のサーバーの日付が残ることがあるので使わない
 */
export async function calendarToday(page: Page): Promise<Date> {
  const [year, month, day] = await page.evaluate(() => {
    const now = new Date();
    return [now.getFullYear(), now.getMonth(), now.getDate()];
  });
  return new Date(year, month, day);
}

/**
 * カレンダー上の予定 (タイトルで探す)。時刻付きの予定は名前が "H:mm タイトル" になるので、
 * 末尾一致で探す (部分一致だと "A" が "A (編集済み)" にも当たってしまう)
 */
export function eventOn(scope: Page | Locator, title: string): Locator {
  const escaped = title.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return scope.getByRole("button", {
    name: new RegExp(`^(?:.* )?${escaped}$`),
  });
}

/**
 * 今日の隣のセル (ドラッグ先)。月表示には翌日のセルがほぼ必ずあるが、
 * 今日がグリッドの最後のセルのときだけ前日にする
 */
export async function neighborDay(page: Page, today: Date): Promise<Date> {
  const tomorrow = addDays(today, 1);
  if ((await dayCell(page, tomorrow).count()) > 0) return tomorrow;
  return addDays(today, -1);
}

/** 予定を別の日付セルへマウスでドラッグする (FullCalendar の interaction プラグインはポインタ操作で動く) */
export async function dragEventTo(page: Page, event: Locator, target: Locator) {
  const from = await event.boundingBox();
  const to = await target.boundingBox();
  if (!from || !to)
    throw new Error("ドラッグ元またはドラッグ先が表示されていません");
  const start = { x: from.x + from.width / 2, y: from.y + from.height / 2 };
  const end = { x: to.x + to.width / 2, y: to.y + to.height / 2 };
  await page.mouse.move(start.x, start.y);
  await page.mouse.down();
  // ドラッグ開始の閾値を超えるために少し動かしてから目的地へ
  await page.mouse.move(start.x + 10, start.y + 10, { steps: 5 });
  await page.mouse.move(end.x, end.y, { steps: 20 });
  await page.mouse.up();
}

/** 「新規作成」からタイトルだけ入れて予定を作る (日時は既定の「今の HH:00 〜 HH:30」) */
export async function createEvent(page: Page, title: string) {
  await page.getByRole("button", { name: "新規作成" }).click();
  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("タイトル").fill(title);
  await dialog.getByRole("button", { name: "作成" }).click();
  await expect(dialog).toBeHidden();
  await expect(eventOn(page, title)).toBeVisible();
}

/** 予定をクリックして概要ポップオーバーを開く */
export async function openEventPopover(page: Page, title: string) {
  await eventOn(page, title).click();
  const popover = page.getByRole("dialog").filter({ hasText: title });
  await expect(popover).toBeVisible();
  return popover;
}
