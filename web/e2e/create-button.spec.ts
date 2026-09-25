import { expect, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

for (const width of [390, 1280]) {
  test(`${width}pxで新規作成と作成メニューを別々に操作できる`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 844 });
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    const create = page.getByRole("button", { name: "新規作成", exact: true });
    const menu = page.getByRole("button", { name: "作成メニューを開く" });
    await create.click();
    const form = page.getByRole("dialog", { name: "予定を作成" });
    await expect(form).toBeVisible();
    await expect(page.getByRole("menu")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(form).toBeHidden();

    await create.focus();
    await page.keyboard.press("Tab");
    await expect(menu).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("menu")).toBeVisible();
    await expect(form).toBeHidden();
    await page
      .getByRole("menuitem", { name: "ICSファイルから取り込む" })
      .click();
    await expect(
      page.getByRole("dialog", { name: "ICSファイルから取り込む" }),
    ).toBeVisible();
    await page.keyboard.press("Escape");

    await create.focus();
    await page.keyboard.press("Enter");
    await expect(form).toBeVisible();
    await page.keyboard.press("Escape");
    await menu.click();
    await page.getByRole("menuitem", { name: /予定を作成/ }).click();
    await expect(form).toBeVisible();
  });
}

test("閲覧専用では新規作成と作成メニューの両方が無効になる", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.member.id}`);
  await expect(
    page.getByRole("button", { name: "新規作成", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "作成メニューを開く" }),
  ).toBeDisabled();
});
