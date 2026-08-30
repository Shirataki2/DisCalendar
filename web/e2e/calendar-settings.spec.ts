import { expect, type Page, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

// カレンダーの表示設定 (#96)。最初に表示するビューと週の開始曜日を選べ、テーマ (#58) と同じく
// 選択は localStorage に残る。ダイアログはアカウントメニューとサイドバーの両方から開ける

const guildId = E2E_GUILDS.admin.id;

/** アカウントメニューから設定ダイアログを開く */
async function openSettingsFromMenu(page: Page) {
  await page.getByRole("button", { name: "アカウントメニュー" }).click();
  await page
    .getByRole("menu")
    .getByRole("menuitem", { name: "カレンダーの表示設定" })
    .click();
  const dialog = page.getByRole("dialog", { name: "カレンダーの表示設定" });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** ビュー切替タブ (ヘッダツールバー) が選択状態か。FullCalendar v7 は role=tablist / tab で出す */
function viewTab(page: Page, name: string) {
  return page.getByRole("tab", { name, exact: true });
}

test("週の開始を月曜にするとその場で反映され、再読込後も残る", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  // 既定は日曜始まり
  await expect(page.getByRole("columnheader").first()).toHaveText("日");

  const dialog = await openSettingsFromMenu(page);
  await dialog.getByRole("combobox", { name: "週の開始曜日" }).click();
  await page.getByRole("option", { name: "月曜日" }).click();
  // ダイアログはモーダルで、開いている間は背景がアクセシビリティツリーから外れる
  // (getByRole で探せない) ので、閉じてから確認する
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  // 再読込しなくてもその場で反映されている
  await expect(page.getByRole("columnheader").first()).toHaveText("月");

  // localStorage に残るので再読込しても月曜始まりのまま
  await page.reload();
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(page.getByRole("columnheader").first()).toHaveText("月");
});

test("最初に表示するビューを「週」にすると、次に開いたときから週になる", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");

  const dialog = await openSettingsFromMenu(page);
  await dialog.getByRole("combobox", { name: "最初に表示するビュー" }).click();
  await page.getByRole("option", { name: "週", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  // 表示中のビューは変わらない (次にカレンダーを開いたときから使われる)
  await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");

  await page.reload();
  await expect(viewTab(page, "週")).toHaveAttribute("aria-selected", "true");
});

test("「前回開いていたビュー」にするとリストで閉じた後もリストで開く", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();

  const dialog = await openSettingsFromMenu(page);
  await dialog.getByRole("combobox", { name: "最初に表示するビュー" }).click();
  await page.getByRole("option", { name: "前回開いていたビュー" }).click();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  await viewTab(page, "リスト").click();
  await expect(page.getByRole("grid")).toHaveCount(0);

  await page.reload();
  await expect(viewTab(page, "リスト")).toHaveAttribute(
    "aria-selected",
    "true",
  );
  // リストビューなので月のグリッドは出ない
  await expect(page.getByRole("grid")).toHaveCount(0);
});

test("サイドバーからも設定を開ける", async ({ page }) => {
  await page.goto("/dashboard");
  await page
    .getByRole("navigation", { name: "サイト内メニュー" })
    .getByRole("button", { name: "カレンダーの表示設定" })
    .click();
  await expect(
    page.getByRole("dialog", { name: "カレンダーの表示設定" }),
  ).toBeVisible();
});
