import { expect, test } from "@playwright/test";

// 404 画面 (app/not-found.tsx、#61)。存在しない URL と、ページが notFound() を投げたときの両方で、
// LP と同じヘッダ・フッタ付きの日本語の画面になり、HTTP ステータスも 404 で返る。
// エラー画面 (app/error.tsx) は描画中の例外を意図的に起こす手段が無いので、ここでは扱わない

const NOT_FOUND_HEADING = { name: "ページが見つかりません" } as const;

test.describe("404 画面", () => {
  test.describe("未ログイン", () => {
    test.use({ storageState: { cookies: [], origins: [] } });

    test("存在しない URL は日本語の 404 画面になる", async ({ page }) => {
      const response = await page.goto("/no-such-page");
      expect(response?.status()).toBe(404);
      await expect(page.getByRole("heading", NOT_FOUND_HEADING)).toBeVisible();
      await expect(page).toHaveTitle(/ページが見つかりません/);
      // LP と同じヘッダ (ログインへの導線) とフッタ
      await expect(
        page.getByRole("banner").getByRole("link", { name: "ログイン" }),
      ).toBeVisible();
      await expect(
        page.getByRole("contentinfo").getByRole("link", { name: "使い方" }),
      ).toBeVisible();

      await page.getByRole("link", { name: "トップページへ" }).click();
      await expect(page).toHaveURL("/");
    });

    test("使い方に無いページも 404 画面になる", async ({ page }) => {
      const response = await page.goto("/docs/no-such-page");
      expect(response?.status()).toBe(404);
      await expect(page.getByRole("heading", NOT_FOUND_HEADING)).toBeVisible();
    });
  });

  test("ログイン済みで不正なサーバー ID を開くと 404 画面になり、サーバー一覧へ戻れる", async ({
    page,
  }) => {
    const response = await page.goto("/dashboard/not-a-guild-id");
    expect(response?.status()).toBe(404);
    await expect(page.getByRole("heading", NOT_FOUND_HEADING)).toBeVisible();

    await page.getByRole("link", { name: "サーバー一覧へ" }).click();
    await expect(page).toHaveURL("/dashboard");
    await expect(
      page.getByRole("heading", { name: "サーバーを選択" }),
    ).toBeVisible();
  });
});
