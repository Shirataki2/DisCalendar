import { expect, type Page, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

// カレンダーのキーボードショートカット (#160)。期間の移動・ビューの切替・新規作成・一覧の表示が
// キーでできて、入力欄にフォーカスがある間や編集権限が無いカレンダーでは効かない

/** ビュー切替タブ (ヘッダツールバー)。FullCalendar v7 は role=tablist / tab で出す */
function viewTab(page: Page, name: string) {
  return page.getByRole("tab", { name, exact: true });
}

/** ツールバーの見出し (表示中の期間。「2026年9月」など) */
function toolbarTitle(page: Page) {
  return page.locator(".calendar-shell").getByRole("heading").first();
}

function shortcutsDialog(page: Page) {
  return page.getByRole("dialog", { name: "キーボードショートカット" });
}

test.describe("編集できるサーバーのカレンダー", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    await expect(page.getByRole("grid")).toBeVisible();
    await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");
  });

  test("m / w / 4 / d / l でビューが切り替わる", async ({ page }) => {
    for (const [key, name] of [
      ["w", "週"],
      ["4", "4日"],
      ["d", "日"],
      ["m", "月"],
    ] as const) {
      await page.keyboard.press(key);
      await expect(viewTab(page, name)).toHaveAttribute(
        "aria-selected",
        "true",
      );
    }
    await page.keyboard.press("l");
    await expect(viewTab(page, "リスト")).toHaveAttribute(
      "aria-selected",
      "true",
    );
    // リストビューなので月のグリッドは出ない
    await expect(page.getByRole("grid")).toHaveCount(0);
  });

  test("← → で期間が動き、t で今日に戻る", async ({ page }) => {
    const initial = await toolbarTitle(page).textContent();
    expect(initial).toBeTruthy();

    await page.keyboard.press("ArrowRight");
    await expect(toolbarTitle(page)).not.toHaveText(initial ?? "");
    const next = await toolbarTitle(page).textContent();

    await page.keyboard.press("ArrowLeft");
    await expect(toolbarTitle(page)).toHaveText(initial ?? "");

    // 2 か月進めてから t で戻る
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowRight");
    await expect(toolbarTitle(page)).not.toHaveText(next ?? "");
    await page.keyboard.press("t");
    await expect(toolbarTitle(page)).toHaveText(initial ?? "");
  });

  test("n で作成ダイアログが開き、入力中はショートカットが効かない", async ({
    page,
  }) => {
    await page.keyboard.press("n");
    const dialog = page.getByRole("dialog", { name: "予定を作成" });
    await expect(dialog).toBeVisible();

    // タイトル欄に w / t / n / l を含む文字を打ってもビューは変わらず、ダイアログも開いたまま
    const title = dialog.getByLabel("タイトル");
    await title.click();
    await title.pressSequentially("twnl4?");
    await expect(title).toHaveValue("twnl4?");
    await expect(dialog).toBeVisible();
    await expect(shortcutsDialog(page)).toHaveCount(0);

    // Esc で閉じる (Base UI の既定)。閉じた後はビューが月のまま
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");
  });

  test("修飾キー付きは奪わない", async ({ page }) => {
    // Ctrl / Cmd / Alt 付きの w はビューを切り替えない (ブラウザのショートカットのまま)
    await page.keyboard.press("Alt+w");
    await page.keyboard.press("Shift+w");
    await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");
    // 修飾キー無しなら切り替わる (リスナー自体は生きている)
    await page.keyboard.press("w");
    await expect(viewTab(page, "週")).toHaveAttribute("aria-selected", "true");
  });

  test("? で一覧が開き、アカウントメニューからも開ける", async ({ page }) => {
    await page.keyboard.press("?");
    await expect(shortcutsDialog(page)).toBeVisible();
    // 一覧には各キーの説明が載っている
    await expect(shortcutsDialog(page)).toContainText("今日に戻る");
    await expect(shortcutsDialog(page)).toContainText("リスト表示に切り替え");
    // 一覧を開いている間はショートカットが効かない
    await page.keyboard.press("w");
    await page.keyboard.press("Escape");
    await expect(shortcutsDialog(page)).toBeHidden();
    await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");

    await page.getByRole("button", { name: "アカウントメニュー" }).click();
    await page
      .getByRole("menu")
      .getByRole("menuitem", { name: "キーボードショートカット" })
      .click();
    await expect(shortcutsDialog(page)).toBeVisible();
  });

  test("ツールバーのボタンの title にキーが併記される", async ({ page }) => {
    await expect(page.getByRole("button", { name: "今日" })).toHaveAttribute(
      "title",
      "今日 (t)",
    );
    await expect(viewTab(page, "週")).toHaveAttribute("title", "週 (w)");
    await expect(viewTab(page, "4日")).toHaveAttribute("title", "4日 (4)");
    await expect(
      page.getByRole("button", { name: "新規作成" }),
    ).toHaveAttribute("title", "新規作成 (n)");
  });
});

test("編集権限が無いサーバーでは n が効かない (他のキーは効く)", async ({
  page,
}) => {
  // member ギルドは restricted で、テストユーザーは一般メンバー
  await page.goto(`/dashboard/${E2E_GUILDS.member.id}`);
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(page.getByRole("button", { name: "新規作成" })).toBeDisabled();

  await page.keyboard.press("n");
  await page.keyboard.press("w");
  await expect(viewTab(page, "週")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("dialog", { name: "予定を作成" })).toHaveCount(0);
});

test("「すべての予定」でも期間の移動とビューの切替ができ、n は効かない", async ({
  page,
}) => {
  await page.goto("/dashboard/all");
  await expect(page.getByRole("grid")).toBeVisible();

  const initial = await toolbarTitle(page).textContent();
  await page.keyboard.press("ArrowRight");
  await expect(toolbarTitle(page)).not.toHaveText(initial ?? "");
  await page.keyboard.press("t");
  await expect(toolbarTitle(page)).toHaveText(initial ?? "");

  await page.keyboard.press("n");
  await page.keyboard.press("d");
  await expect(viewTab(page, "日")).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test.describe("スマートフォンの幅", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true });

  test("アカウントメニューに「キーボードショートカット」を出さない", async ({
    page,
  }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    await expect(page.getByRole("grid")).toBeVisible();
    await page.getByRole("button", { name: "アカウントメニュー" }).tap();
    const menu = page.getByRole("menu");
    await expect(
      menu.getByRole("menuitem", { name: "カレンダーの表示設定" }),
    ).toBeVisible();
    await expect(
      menu.getByRole("menuitem", { name: "キーボードショートカット" }),
    ).toBeHidden();
  });
});
