import { expect, type Page, test } from "@playwright/test";
import { version } from "../package.json";
import { E2E_GUILDS, E2E_USER } from "./fixtures";

// ダッシュボードのナビゲーションドロワーと固定フッタ (#51)。
// PC 幅では常設サイドバー (ハンバーガーで開閉、状態は cookie に残る)、スマホ幅ではオーバーレイのドロワー

/** ドロワー / サイドバーに並ぶリンク (旧版の NavDrawer.vue と同じ並び)。管理コンソールは管理者でないので出ない */
const NAV_LINKS = [
  { name: "ホーム", href: "/" },
  { name: "サーバー一覧", href: "/dashboard" },
  { name: "サポートサーバー", href: /^https:\/\/discord\.gg\// },
  { name: "使い方", href: "/docs/gettingstarted" },
  { name: "利用規約", href: "/support/tos" },
  { name: "プライバシーポリシー", href: "/support/privacy" },
  { name: "GitHub", href: /^https:\/\/github\.com\// },
] as const;

function sidebar(page: Page) {
  return page.getByRole("navigation", { name: "サイト内メニュー" });
}

async function expectNavLinks(page: Page) {
  const nav = sidebar(page);
  for (const link of NAV_LINKS) {
    await expect(nav.getByRole("link", { name: link.name })).toHaveAttribute(
      "href",
      link.href,
    );
  }
  await expect(nav.getByRole("link", { name: "管理コンソール" })).toHaveCount(
    0,
  );
  await expect(nav.getByRole("button", { name: "ログアウト" })).toBeVisible();
}

test.describe("PC 幅", () => {
  test("サイドバーにリンクが並び、現在のページが強調され、フッタにバージョンが出る", async ({
    page,
  }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    await expect(sidebar(page)).toBeVisible();
    await expectNavLinks(page);
    // カレンダー (/dashboard/<id>) も「サーバー一覧」の配下として現在地扱い
    await expect(
      sidebar(page).getByRole("link", { name: "サーバー一覧" }),
    ).toHaveAttribute("aria-current", "page");
    await expect(
      sidebar(page).getByRole("link", { name: "ホーム" }),
    ).not.toHaveAttribute("aria-current", "page");

    const footer = page.getByRole("contentinfo");
    await expect(footer).toContainText(`v ${version}`);
    await expect(footer).toContainText("© 2021");
  });

  test("ハンバーガーでサイドバーを閉じると再読込後も閉じたまま、開き直せる", async ({
    page,
  }) => {
    await page.goto("/dashboard");
    const menu = page.getByRole("button", { name: "メニュー", exact: true });
    await expect(sidebar(page)).toBeVisible();
    await expect(menu).toHaveAttribute("aria-expanded", "true");

    await menu.click();
    await expect(sidebar(page)).toBeHidden();
    await expect(menu).toHaveAttribute("aria-expanded", "false");

    // cookie に覚えているので再読込しても閉じたまま
    await page.reload();
    await expect(
      page.getByRole("heading", { name: "サーバーを選択" }),
    ).toBeVisible();
    await expect(sidebar(page)).toBeHidden();

    await menu.click();
    await expect(sidebar(page)).toBeVisible();
  });
});

test.describe("スマホ幅", () => {
  test.use({ viewport: { width: 375, height: 812 } });

  test("ハンバーガーでドロワーが開き、リンクを押すと閉じて遷移する", async ({
    page,
  }) => {
    await page.goto("/dashboard");
    // 常設サイドバーは出ない
    await expect(sidebar(page)).toBeHidden();

    await page.getByRole("button", { name: "メニュー", exact: true }).click();
    const drawer = page.getByRole("dialog");
    await expect(drawer).toBeVisible();
    await expectNavLinks(page);

    await drawer.getByRole("link", { name: "使い方" }).click();
    await expect(page).toHaveURL(/\/docs\/gettingstarted$/);
    await expect(drawer).toBeHidden();
  });

  test("ヘッダのアカウントメニューにユーザー名とログアウトが出る", async ({
    page,
  }) => {
    await page.goto("/dashboard");
    await page.getByRole("button", { name: "アカウントメニュー" }).click();
    const menu = page.getByRole("menu");
    await expect(menu).toBeVisible();
    // スマホ幅ではヘッダに名前を出さない代わりにメニューの先頭に出す
    await expect(menu).toContainText(E2E_USER.name);
    await expect(
      menu.getByRole("menuitem", { name: "サーバー一覧" }),
    ).toHaveAttribute("href", "/dashboard");
    await expect(
      menu.getByRole("menuitem", { name: "ログアウト" }),
    ).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(menu).toBeHidden();
  });
});
