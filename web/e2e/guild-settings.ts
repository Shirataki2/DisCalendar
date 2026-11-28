import { expect, type Locator, type Page } from "@playwright/test";

/** サーバー設定ページの節 (左の一覧のリンク名) */
export type GuildSettingsSection =
  | "全般"
  | "外部カレンダー"
  | "Webhook・API トークン"
  | "削除した予定";

/**
 * カレンダー画面の歯車からサーバー設定ページ (`/dashboard/[id]/settings`) を開き、必要なら節を切り替える。
 * 返すのはページの本文 (main)。ダイアログだった頃の `getByRole("dialog")` の代わりに使う
 */
export async function openGuildSettings(
  page: Page,
  section: GuildSettingsSection = "全般",
): Promise<Locator> {
  await page.getByRole("link", { name: "サーバー設定", exact: true }).click();
  await expect(
    page.getByRole("heading", { level: 1, name: "サーバー設定" }),
  ).toBeVisible();
  if (section !== "全般") await gotoGuildSettingsSection(page, section);
  return page.getByRole("main");
}

/** サーバー設定ページの中で節を切り替える */
export async function gotoGuildSettingsSection(
  page: Page,
  section: GuildSettingsSection,
): Promise<void> {
  const link = page
    .getByRole("navigation", { name: "サーバー設定の項目" })
    .getByRole("link", { name: section, exact: true });
  await link.click();
  await expect(link).toHaveAttribute("aria-current", "page");
}

/** サーバー設定ページからカレンダー画面に戻る */
export async function backToCalendar(page: Page): Promise<void> {
  await page.getByRole("link", { name: "カレンダーに戻る" }).click();
  await expect(
    page.getByRole("link", { name: "サーバー設定", exact: true }),
  ).toBeVisible();
}
