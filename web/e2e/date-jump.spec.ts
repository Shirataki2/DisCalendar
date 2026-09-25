import { expect, type Page, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

// ツールバーの「日付を指定して移動」(#59)。日付ピッカーで選んだ日付を含む期間へ、
// いまのビューのまま移動する

function viewTab(page: Page, name: string) {
  return page.getByRole("tab", { name, exact: true });
}

/** ツールバーの見出し (表示中の期間。「2026年9月」など) */
function toolbarTitle(page: Page) {
  return page.locator(".calendar-shell").getByRole("heading").first();
}

async function openDateJump(page: Page) {
  await page
    .locator(".calendar-shell")
    .getByRole("button", { name: "日付を指定して移動" })
    .click();
  const picker = page.getByRole("dialog");
  await expect(picker.getByRole("grid")).toBeVisible();
  return picker;
}

test.beforeEach(async ({ page }) => {
  await page.clock.setFixedTime(new Date("2026-09-15T12:00:00+09:00"));
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");
  await expect(toolbarTitle(page)).toHaveText("2026年9月");
});

test("年と月を選んで離れた月へ移動する", async ({ page }) => {
  const picker = await openDateJump(page);
  // 年・月のプルダウンは開いた時点の表示位置 (2026年9月) から始まる
  const yearSelect = picker.getByRole("combobox", { name: "年を選択" });
  const monthSelect = picker.getByRole("combobox", { name: "月を選択" });
  await expect(yearSelect).toHaveValue("2026");
  await expect(monthSelect).toHaveValue("8"); // 0 始まり (9月)
  await yearSelect.selectOption("2028");
  await monthSelect.selectOption({ label: "3月" });
  await picker.getByRole("button", { name: /2028年3月20日/ }).click();

  await expect(picker).toBeHidden();
  await expect(toolbarTitle(page)).toHaveText("2028年3月");
  await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");

  // もう一度開くと、移動後の日付が選ばれた状態から始まる
  const reopened = await openDateJump(page);
  await expect(
    reopened.getByRole("combobox", { name: "年を選択" }),
  ).toHaveValue("2028");
});

test("週表示では選んだ日を含む週へ移動する", async ({ page }) => {
  await viewTab(page, "週").click();
  const picker = await openDateJump(page);
  await picker.getByRole("button", { name: /2026年9月30日/ }).click();

  await expect(toolbarTitle(page)).toHaveText("2026年9/27–10/3");
  await expect(viewTab(page, "週")).toHaveAttribute("aria-selected", "true");
});
