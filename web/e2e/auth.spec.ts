import { expect, test } from "@playwright/test";
import { BETTER_AUTH_SECRET, DATABASE_URL } from "./env";
import { E2E_GUILDS, E2E_USER } from "./fixtures";
import { createExtraSession, sessionCookie } from "./seed";

// ログインとサーバー選択。セッションは global-setup.ts が DB に作った cookie で、Discord OAuth は通さない

test.describe("未ログイン", () => {
  // storageState を空にしてログインしていない状態にする
  test.use({ storageState: { cookies: [], origins: [] } });

  test("/dashboard はログイン画面へリダイレクトされる", async ({ page }) => {
    await page.goto("/dashboard");
    await expect(page).toHaveURL(/\/login$/);
    await expect(
      page.getByRole("button", { name: "Discordでログイン" }),
    ).toBeVisible();
  });

  test("ギルドのカレンダーも開けない", async ({ page }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    await expect(page).toHaveURL(/\/login$/);
  });
});

test.describe("ログイン済み", () => {
  test("サーバー選択に Bot 参加済みのサーバーと招待できるサーバーが出る", async ({
    page,
  }) => {
    await page.goto("/dashboard");
    await expect(
      page.getByRole("heading", { name: "サーバーを選択" }),
    ).toBeVisible();
    // ヘッダにユーザー名
    await expect(page.getByText(E2E_USER.name)).toBeVisible();

    // Bot 参加済み → カレンダーへのリンク
    const admin = page.getByRole("link", { name: E2E_GUILDS.admin.name });
    await expect(admin).toHaveAttribute(
      "href",
      `/dashboard/${E2E_GUILDS.admin.id}`,
    );
    await expect(
      page.getByRole("link", { name: E2E_GUILDS.member.name }),
    ).toHaveAttribute("href", `/dashboard/${E2E_GUILDS.member.id}`);

    // 管理権限があって Bot 未参加 → 招待リンク (Discord の Bot 追加画面)
    await expect(
      page.getByRole("heading", { name: "Bot を招待できるサーバー" }),
    ).toBeVisible();
    const invitable = page.getByRole("link", {
      name: new RegExp(E2E_GUILDS.invitable.name),
    });
    await expect(invitable).toHaveAttribute(
      "href",
      new RegExp(
        `^https://discord\\.com/oauth2/authorize\\?.*guild_id=${E2E_GUILDS.invitable.id}`,
      ),
    );
    await expect(invitable).toHaveAttribute("target", "_blank");
  });

  test("サーバーを選ぶとそのカレンダーが開く", async ({ page }) => {
    await page.goto("/dashboard");
    await page.getByRole("link", { name: E2E_GUILDS.admin.name }).click();
    await expect(page).toHaveURL(`/dashboard/${E2E_GUILDS.admin.id}`);
    await expect(page.getByText(E2E_GUILDS.admin.name)).toBeVisible();
    // FullCalendar の月表示と「新規作成」
    await expect(page.getByRole("grid")).toBeVisible();
    await expect(page.getByRole("tab", { name: "月" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(page.getByRole("button", { name: "新規作成" })).toBeEnabled();
  });

  test("ログアウトするとログイン画面に戻り、セッションが無効になる", async ({
    browser,
  }) => {
    // 共有のセッション (storageState) を消さないよう、このテスト専用のセッションでログインする
    const token = await createExtraSession(DATABASE_URL);
    const context = await browser.newContext({
      storageState: { cookies: [], origins: [] },
    });
    await context.addCookies([sessionCookie(token, BETTER_AUTH_SECRET)]);
    const page = await context.newPage();

    await page.goto("/dashboard");
    await expect(
      page.getByRole("heading", { name: "サーバーを選択" }),
    ).toBeVisible();
    // ログアウトはヘッダ右端のアカウントメニュー (アバター) のドロップダウンから
    await page.getByRole("button", { name: "アカウントメニュー" }).click();
    await page.getByRole("menuitem", { name: "ログアウト" }).click();
    await expect(page).toHaveURL(/\/login$/);
    // cookie が消えた / セッションが無効になったので開き直しても入れない
    await page.goto("/dashboard");
    await expect(page).toHaveURL(/\/login$/);
    await context.close();
  });
});
