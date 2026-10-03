import { expect, test } from "@playwright/test";
import { calendarToday, dayCell, eventOn, openEventPopover } from "./calendar";
import { E2E_EDITOR_ROLES, E2E_GUILDS } from "./fixtures";

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

test("メンションの検証・非参加・API エラーも別タブの言語切り替えに追従する", async ({
  page,
  context,
}) => {
  await page.route("**/local/api/guilds/*/mention-members?*", (route) =>
    route.fulfill(
      route.request().url().includes("888888888888888888")
        ? { status: 403, json: { error: "forbidden", message: "test" } }
        : {
            json: [
              {
                user_id: "999999999999999999",
                display_name: null,
                avatar_url: null,
              },
            ],
          },
    ),
  );
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await page
    .getByRole("combobox", { name: "Language / 言語" })
    .selectOption("en");
  await page.getByRole("button", { name: "New event", exact: true }).click();
  await page.getByRole("tab", { name: /^Repeat & reminders/ }).click();
  const form = page.getByRole("dialog");
  await form
    .getByRole("checkbox", { name: "@everyone (everyone)", exact: true })
    .check();
  await form.getByRole("combobox", { name: "Role to mention" }).click();
  await page
    .getByRole("option", { name: E2E_EDITOR_ROLES[0].name, exact: true })
    .click();
  const other = await context.newPage();
  await other.goto("/dashboard");
  const language = other.getByRole("combobox", { name: "Language / 言語" });
  for (const [id, english, japanese] of [
    ["abc", "Enter a valid user ID", "正しいユーザーIDを入力してください"],
    [
      "999999999999999999",
      "This user is not a member of this server",
      "このユーザーはサーバーに参加していません",
    ],
    [
      "888888888888888888",
      "You do not have permission to do this",
      "この操作を行う権限がありません",
    ],
  ]) {
    await form.getByLabel("User ID to mention", { exact: true }).fill(id);
    const heading = form.getByRole("heading", { name: /^Reminder mentions/ });
    await expect(heading).toContainText("(entering a user ID)");
    await expect(heading).toContainText(
      `@everyone, @${E2E_EDITOR_ROLES[0].name}`,
    );
    await form.getByRole("button", { name: "Add user", exact: true }).click();
    await expect(form.getByText(english, { exact: true })).toBeVisible();
    await language.selectOption("ja");
    await expect(form.getByText(japanese, { exact: true })).toBeVisible();
    const japaneseHeading = form.getByRole("heading", {
      name: /^通知のメンション先/,
    });
    await expect(japaneseHeading).toContainText("（ユーザーIDを入力中）");
    await expect(japaneseHeading).toContainText(
      `@everyone、@${E2E_EDITOR_ROLES[0].name}`,
    );
    await expect(
      form.getByLabel("メンションするユーザーID", { exact: true }),
    ).toHaveValue(id);
    await language.selectOption("en");
    await expect(form.getByText(english, { exact: true })).toBeVisible();
  }
});

test("ページタイトルも言語切り替え・クライアント遷移・戻る操作に追従する", async ({
  page,
}) => {
  await page.goto("/dashboard");
  const language = page.getByRole("combobox", { name: "Language / 言語" });
  await expect(page).toHaveTitle("サーバー選択 | DisCalendar");
  await language.selectOption("en");
  await expect(page).toHaveTitle("Choose a server | DisCalendar");
  await page
    .locator(`a[href="/dashboard/${E2E_GUILDS.admin.id}"]`)
    .first()
    .click();
  await expect(
    page.getByRole("button", { name: "New event", exact: true }),
  ).toBeVisible();
  await expect(page).toHaveTitle("Calendar | DisCalendar");
  await page.locator('a[href="/dashboard/all"]').first().click();
  await expect(
    page.getByRole("heading", { name: "All events", exact: true }),
  ).toBeVisible();
  await expect(page).toHaveTitle("All events | DisCalendar");
  await language.selectOption("ja");
  await expect(page).toHaveTitle("すべての予定 | DisCalendar");
  await language.selectOption("en");
  await page.goBack();
  await expect(page).toHaveTitle("Calendar | DisCalendar");
  await page.reload();
  await expect(page).toHaveTitle("Calendar | DisCalendar");
  await language.selectOption("ja");
  await expect(page).toHaveTitle("カレンダー | DisCalendar");
  await page.getByRole("link", { name: "サーバー一覧へ", exact: true }).click();
  await expect(page).toHaveTitle("サーバー選択 | DisCalendar");
});

test("英語の繰り返し終了日を空にしても検証エラーから復帰できる", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await page
    .getByRole("combobox", { name: "Language / 言語" })
    .selectOption("en");
  await page.getByRole("button", { name: "New event", exact: true }).click();
  await page.getByRole("tab", { name: /^Repeat & reminders/ }).click();
  await page
    .getByRole("button", { name: "Does not repeat", exact: true })
    .click();
  const settings = page.getByRole("dialog", { name: "Repeat settings" });
  await settings.getByLabel("Repeat frequency").selectOption("daily");
  await settings.getByRole("radio", { name: "End date", exact: true }).check();
  const date = settings.getByLabel("Repeat end date", { exact: true });
  const validDate = await date.inputValue();
  await date.fill("");
  await expect(settings).toContainText("Daily / select a valid end date");
  await expect(settings.getByRole("alert")).toBeVisible();
  await expect(
    settings.getByRole("button", { name: "Apply settings" }),
  ).toBeDisabled();
  await date.fill(validDate);
  await expect(
    settings.getByRole("button", { name: "Apply settings" }),
  ).toBeEnabled();
  await settings.getByRole("button", { name: "Apply settings" }).click();
  await expect(settings).toBeHidden();
  await expect(
    page.getByRole("dialog", { name: "Create event" }),
  ).toBeVisible();
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
    const unit = form
      .getByRole("combobox", { name: "Reminder timing (unit)" })
      .first();
    await expect(unit).toContainText("day before");
    await expect(
      form.getByRole("combobox", { name: "Reminder timing (unit)" }).nth(1),
    ).toContainText("hour before");
    await number.fill("2");
    await expect(unit).toContainText("days before");
    await number.fill("1");
    await expect(unit).toContainText("day before");
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

test("英語選択中も未翻訳の日程調整の日付とルート404本文を日本語で扱う", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.polls.id}/polls`);
  await page
    .getByRole("combobox", { name: "Language / 言語" })
    .selectOption("en");
  await page
    .getByRole("button", { name: "日程調整を作成", exact: true })
    .click();
  const form = page.getByRole("dialog", {
    name: "日程調整を作成",
    exact: true,
  });
  await expect(form).toHaveAttribute("lang", "ja");
  const date = form.getByRole("button", { name: "候補1の開始日", exact: true });
  await expect(date).toHaveText(/\d{4}\/\d{2}\/\d{2} \([日月火水木金土]\)/);
  await date.click();
  const calendar = page
    .getByRole("dialog")
    .filter({ has: page.getByRole("grid") });
  await expect(calendar).toHaveAttribute("lang", "ja");
  await expect(calendar).toContainText(/\d{4}年/);
  await page.goto("/no-such-page");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.locator("main")).toHaveAttribute("lang", "ja");
  await expect(
    page.getByRole("heading", { name: "ページが見つかりません" }),
  ).toBeVisible();
});
