import { expect, test } from "@playwright/test";
import { createEvent, openEventPopover } from "./calendar";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS } from "./fixtures";
import { deleteEventsNamed } from "./seed";

const eventName = "切り替え前だけにある予定";

test.afterEach(async () => {
  await deleteEventsNamed(DATABASE_URL, [eventName]);
});

test("一覧に確実な権限区分が表示される", async ({ page }) => {
  await page.goto("/dashboard");
  const main = page.getByRole("main");
  await expect(
    main.getByRole("link", { name: new RegExp(E2E_GUILDS.admin.name) }),
  ).toContainText("オーナー");
  await expect(
    main.getByRole("link", { name: new RegExp(E2E_GUILDS.member.name) }),
  ).toContainText("メンバー");
});

test("キーボードで別サーバーへ切り替えると開いていた予定を残さない", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await createEvent(page, eventName);
  await openEventPopover(page, eventName);

  const trigger = page.getByRole("button", {
    name: `サーバーを切り替え: ${E2E_GUILDS.admin.name}`,
  });
  await trigger.focus();
  await page.keyboard.press("Enter");
  const destination = page.getByRole("menuitem", {
    name: new RegExp(E2E_GUILDS.member.name),
  });
  await destination.focus();
  await page.keyboard.press("Enter");

  await expect(page).toHaveURL(`/dashboard/${E2E_GUILDS.member.id}`);
  await expect(
    page.getByRole("button", {
      name: `サーバーを切り替え: ${E2E_GUILDS.member.name}`,
    }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "新規作成" })).toBeDisabled();
  await expect(
    page.getByRole("dialog").filter({ hasText: eventName }),
  ).toHaveCount(0);
});

test("狭い画面でもサーバー候補を選べる", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await page
    .getByRole("button", {
      name: `サーバーを切り替え: ${E2E_GUILDS.admin.name}`,
    })
    .click();

  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  const box = await menu.boundingBox();
  expect(box?.x).toBeGreaterThanOrEqual(0);
  expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(375);
  const longName = menu.getByTitle(E2E_GUILDS.editorRoles.name);
  await expect(longName).toHaveCSS("text-overflow", "ellipsis");
  expect(
    await longName.evaluate(
      (element) => element.scrollWidth > element.clientWidth,
    ),
  ).toBe(true);
  await menu
    .getByRole("menuitem", { name: new RegExp(E2E_GUILDS.member.name) })
    .click();
  await expect(page).toHaveURL(`/dashboard/${E2E_GUILDS.member.id}`);
});
