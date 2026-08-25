import { expect, type Page, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

// テーマ切替 (#58)。ダッシュボードだけダーク / ライトを選べ、選択は localStorage に残る。
// ライトの配色を用意していない LP・使い方・ログインはダーク固定

/**
 * ドロワー (サイドバー) の切替ボタン。ラベルは「今のテーマ」ではなく「切り替え先」を出す。
 * 両方のラベルが DOM にあり CSS で出し分けているので、隠れている方を除く読み上げ名で選ぶ
 */
function themeToggle(page: Page, name: string) {
  return page
    .getByRole("navigation", { name: "サイト内メニュー" })
    .getByRole("button", { name });
}

test("ドロワーからライトに切り替えると再読込後も残り、LP はダークのまま", async ({
  page,
}) => {
  await page.goto("/dashboard");
  const html = page.locator("html");
  await expect(html).toHaveAttribute("data-color-scheme", "dark");
  await expect(html).toHaveClass(/(^|\s)dark(\s|$)/);

  await themeToggle(page, "ライトテーマに切り替え").click();
  await expect(html).toHaveAttribute("data-color-scheme", "light");
  await expect(html).not.toHaveClass(/(^|\s)dark(\s|$)/);
  // 押した後は逆向きのラベルになる
  await expect(themeToggle(page, "ダークテーマに切り替え")).toBeVisible();

  // localStorage に残るので再読込しても、カレンダーの画面に移ってもライトのまま
  await page.reload();
  await expect(html).toHaveAttribute("data-color-scheme", "light");
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(html).toHaveAttribute("data-color-scheme", "light");

  // LP はライトの配色を用意していないのでダークに固定される (選択は消えない)
  await page.goto("/");
  await expect(html).toHaveAttribute("data-color-scheme", "dark");
  await page.goto("/dashboard");
  await expect(html).toHaveAttribute("data-color-scheme", "light");
});

test("アカウントメニューからも切り替えられる", async ({ page }) => {
  await page.goto("/dashboard");
  const html = page.locator("html");
  await page.getByRole("button", { name: "アカウントメニュー" }).click();
  await page
    .getByRole("menu")
    .getByRole("menuitem", { name: "ライトテーマに切り替え" })
    .click();

  await expect(html).toHaveAttribute("data-color-scheme", "light");
});
