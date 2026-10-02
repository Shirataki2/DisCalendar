import { expect, test } from "@playwright/test";
import { calendarToday, dayCell, eventOn, openEventPopover } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

test.describe("初回の言語とブラウザ保存", () => {
  test.use({ storageState: { cookies: [], origins: [] }, locale: "en-US" });

  test("英語ログイン、キーボード切り替え、再読み込み、別タブを検証する", async ({
    page,
    context,
  }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    await page.goto("/login");
    const language = page.getByRole("combobox", { name: "Language / 言語" });
    await expect(
      page.getByRole("button", { name: "Sign in with Discord" }),
    ).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    expect(
      await page.evaluate(
        () =>
          document.documentElement.scrollWidth -
          document.documentElement.clientWidth,
      ),
    ).toBeLessThanOrEqual(0);
    await language.focus();
    await expect(language).toBeFocused();
    await language.press("Home");
    await language.press("Enter");
    await expect(language).toHaveValue("ja");
    await expect(
      page.getByRole("button", { name: "Discordでログイン" }),
    ).toBeVisible();
    await page.reload();
    await expect(language).toHaveValue("ja");
    const other = await context.newPage();
    await other.goto("/login");
    await language.selectOption("en");
    await expect(
      other.getByRole("button", { name: "Sign in with Discord" }),
    ).toBeVisible();
    for (const path of ["/", "/donation", "/tutorial"]) {
      await page.goto(path);
      await expect(page.locator("main")).toHaveAttribute("lang", "ja");
    }
  });

  test("保存できない環境でも言語を変更できる", async ({ page }) => {
    await page.addInitScript(() => {
      Storage.prototype.getItem = () => {
        throw new DOMException("Disabled", "SecurityError");
      };
      Storage.prototype.setItem = () => {
        throw new DOMException("Disabled", "SecurityError");
      };
    });
    await page.goto("/login");
    await expect(
      page.getByRole("button", { name: "Sign in with Discord" }),
    ).toBeVisible();
    await page
      .getByRole("combobox", { name: "Language / 言語" })
      .selectOption("ja");
    await expect(
      page.getByRole("button", { name: "Discordでログイン" }),
    ).toBeVisible();
  });
});

test.describe("未対応言語", () => {
  test.use({ storageState: { cookies: [], origins: [] }, locale: "fr-FR" });
  test("日本語にフォールバックする", async ({ page }) => {
    await page.goto("/login");
    await expect(
      page.getByRole("button", { name: "Discordでログイン" }),
    ).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "ja");
  });
});

for (const width of [375, 390, 1280]) {
  test(`${width}px: 英語の予定作成・エラー・編集・複製・削除と横断カレンダー`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 844 });
    const noon = new Date();
    noon.setUTCHours(3, 0, 0, 0);
    await page.clock.setFixedTime(noon);
    await page.goto("/dashboard");
    await page
      .getByRole("combobox", { name: "Language / 言語" })
      .selectOption("en");
    await expect(
      page.getByRole("heading", { name: "Choose a server" }),
    ).toBeVisible();
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    await expect(
      page.getByRole("tab", { name: "Month view", exact: true }),
    ).toBeVisible();
    expect(
      await page.evaluate(
        () =>
          document.documentElement.scrollWidth -
          document.documentElement.clientWidth,
      ),
    ).toBeLessThanOrEqual(0);
    const today = await calendarToday(page);
    await dayCell(page, today).click({ position: { x: 12, y: 45 } });
    const quickAdd = page.getByRole("dialog", { name: "Quick add event" });
    await quickAdd
      .getByRole("textbox", { name: "Title", exact: true })
      .fill("   ");
    await quickAdd
      .getByRole("textbox", { name: "Title", exact: true })
      .press("Enter");
    await expect(quickAdd.getByRole("alert")).toHaveText("Enter a title");
    await page.keyboard.press("Escape");
    await expect(quickAdd).toBeHidden();
    await page.getByRole("button", { name: "New event", exact: true }).click();
    const form = page.getByRole("dialog", { name: "Create event" });
    const title = `日英テスト ${width} ${Date.now().toString(36)}`;
    await form.getByRole("button", { name: "Create", exact: true }).click();
    await expect(
      form.getByText("Enter a title", { exact: true }),
    ).toBeVisible();
    await expect(
      form.getByRole("textbox", { name: "Title", exact: true }),
    ).toBeFocused();
    await form.getByRole("textbox", { name: "Title", exact: true }).fill(title);
    await form
      .getByRole("textbox", { name: "Description", exact: true })
      .fill("利用者の説明は翻訳しない");
    await form.getByRole("tab", { name: /^Repeat & reminders/ }).click();
    const number = form.getByLabel("Reminder timing (number)").first();
    await number.fill("0");
    await form.getByRole("button", { name: "Create", exact: true }).click();
    await expect(
      form.getByText("Enter a number from 1 to 100", { exact: true }),
    ).toBeVisible();
    await number.fill("1");
    for (const name of [/^Basic/, /^Repeat & reminders/, /^Files & sharing/]) {
      const tab = form.getByRole("tab", { name });
      await tab.click();
      expect(await tab.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(
        true,
      );
      await expect(
        form.getByRole("button", { name: "Create", exact: true }),
      ).toBeInViewport();
      expect(
        await form.evaluate((el) => el.scrollWidth <= el.clientWidth),
      ).toBe(true);
    }
    await page.screenshot({ path: `/tmp/language-form-${width}.png` });
    await form.getByRole("button", { name: "Create", exact: true }).click();
    await expect(form).toBeHidden();
    const popover = await openEventPopover(page, title);
    await expect(popover).toContainText("1 day before");
    await expect(popover).toContainText("利用者の説明は翻訳しない");
    await popover.getByRole("button", { name: "Edit", exact: true }).click();
    const edit = page.getByRole("dialog", { name: "Edit event" });
    await expect(
      edit.getByRole("textbox", { name: "Title", exact: true }),
    ).toHaveValue(title);
    await edit
      .getByRole("textbox", { name: "Title", exact: true })
      .fill(`${title} edit`);
    await edit.getByRole("button", { name: "Save", exact: true }).click();
    await expect(edit).toBeHidden();
    const updated = await openEventPopover(page, `${title} edit`);
    await updated
      .getByRole("button", { name: "Duplicate", exact: true })
      .click();
    const copy = page.getByRole("dialog", { name: "Duplicate event" });
    await expect(
      copy.getByRole("textbox", { name: "Title", exact: true }),
    ).toHaveValue(`${title} edit`);
    await copy.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(copy).toBeHidden();
    await page.goto("/dashboard/all");
    await expect(
      page.getByRole("heading", { name: "All events" }),
    ).toBeVisible();
    await expect(eventOn(page, `${title} edit`)).toBeVisible();
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    const remove = await openEventPopover(page, `${title} edit`);
    await remove.getByRole("button", { name: "Delete", exact: true }).click();
    const confirmation = page.getByRole("alertdialog");
    await expect(confirmation).toContainText(`${title} edit`);
    await confirmation
      .getByRole("button", { name: "Delete", exact: true })
      .click();
    await expect(eventOn(page, `${title} edit`)).toHaveCount(0);
    await page
      .getByRole("combobox", { name: "Language / 言語" })
      .selectOption("ja");
    await expect(
      page.getByRole("button", { name: "新規作成", exact: true }),
    ).toBeVisible();
  });
}
