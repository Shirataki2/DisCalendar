import { expect, test } from "@playwright/test";

// 支援ページ。決済ページ・カスタマーポータルの URL は実行時の環境変数で決まるので、
// ページのリンクは自サイトの Route Handler (/donation/*) を指す。飛び先 (Stripe) までは開かない

test("毎月・1 回だけの支援と、支援の管理へのリンクが並ぶ", async ({ page }) => {
  await page.goto("/donation");
  await expect(
    page.getByRole("heading", { level: 1, name: "支援のお願い" }),
  ).toBeVisible();
  await expect(
    page.getByRole("link", { name: "毎月支援する" }),
  ).toHaveAttribute("href", "/donation/monthly");
  await expect(
    page.getByRole("link", { name: "1 回だけ支援する" }),
  ).toHaveAttribute("href", "/donation/checkout");
  await expect(
    page.getByRole("link", { name: "支援の管理 (解約・カード変更)" }),
  ).toHaveAttribute("href", "/donation/portal");
});

test("英語表示でもボタンが訳される", async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem("discalendar-language", "en"),
  );
  await page.goto("/donation");
  await expect(
    page.getByRole("link", { name: "Donate monthly" }),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: "Donate once" })).toBeVisible();
  await expect(
    page.getByRole("link", { name: "Manage donation (cancel or update card)" }),
  ).toBeVisible();
});
