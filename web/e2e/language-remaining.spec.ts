import { expect, type Page, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

const guild = E2E_GUILDS.admin.id;
async function english(page: Page) {
  await page.addInitScript(() =>
    localStorage.setItem("discalendar-language", "en"),
  );
}
async function noOverflow(page: Page) {
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
}

for (const width of [390, 1280]) {
  test(`公開案内と全使い方ページの英語・読み上げ・横幅 (${width}px)`, async ({
    page,
  }) => {
    await english(page);
    await page.setViewportSize({ width, height: 844 });
    for (const [path, heading] of [
      ["/", "Features"],
      ["/tutorial", "Try the calendar"],
      ["/donation", "Support DisCalendar"],
      ["/no-such-page", "Page not found"],
      ["/mcp/connections", "MCP connections"],
    ]) {
      await page.goto(path);
      await expect(
        page.getByRole("heading", { name: heading, exact: true }),
      ).toBeVisible();
      if (path === "/tutorial")
        await expect(
          page.getByRole("complementary", { name: "Guide", exact: true }),
        ).toBeVisible();
      await noOverflow(page);
    }
    for (const slug of [
      "gettingstarted",
      "login",
      "invite",
      "initialize",
      "calendar",
      "polls",
      "edit",
      "subscribe",
      "webhooks",
      "mcp",
      "commands",
    ]) {
      await page.goto(`/docs/${slug}`);
      await expect(page.locator("article > div[lang=en]")).toBeVisible();
      await expect(page.locator("article > div[lang=ja]")).toHaveCount(0);
      await noOverflow(page);
    }
    await page.screenshot({
      path: test.info().outputPath(`guide-${width}.png`),
      fullPage: true,
    });
  });
}

test("設定の編集中も別タブで言語を切り替え、値とAPIエラーを保持する", async ({
  page,
  context,
}) => {
  await english(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`/dashboard/${guild}`);
  const other = await context.newPage();
  await other.goto("/login");
  await page
    .getByRole("button", { name: "Server settings", exact: true })
    .click();
  const dialog = page.getByRole("dialog");
  const external = dialog.locator(
    "section[aria-labelledby=external-calendar-heading]",
  );
  await external
    .getByLabel("Display name", { exact: true })
    .fill("翻訳しない名前");
  await external.getByLabel("ICS URL").fill("https://example.com/calendar.ics");
  await page.route(`**/guilds/${guild}/external-calendars`, (route) =>
    route.request().method() === "POST"
      ? route.fulfill({
          status: 400,
          json: { error: "bad_request", message: "この URL は登録済みです" },
        })
      : route.continue(),
  );
  await external.getByRole("button", { name: "Add", exact: true }).click();
  await expect(external.getByRole("alert")).toContainText(
    "This URL is already registered",
  );
  await other
    .getByRole("combobox", { name: "Language / 言語" })
    .selectOption("ja");
  await expect(dialog).toHaveAttribute("lang", "ja");
  await expect(external.getByRole("alert")).toContainText(
    "この URL は登録済みです",
  );
  await expect(dialog.getByLabel("表示名", { exact: true })).toHaveValue(
    "翻訳しない名前",
  );
  await other
    .getByRole("combobox", { name: "Language / 言語" })
    .selectOption("en");
  await expect(dialog).toHaveAttribute("lang", "en");
  await expect(
    external.getByLabel("Display name", { exact: true }),
  ).toHaveValue("翻訳しない名前");
  await noOverflow(page);
  await page.screenshot({ path: test.info().outputPath("settings-390.png") });
});

test("英語でICSを取り込み、公開共有ページでも内容とJSTを保持する", async ({
  page,
  browser,
  baseURL,
}) => {
  await english(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`/dashboard/${guild}`);
  await page.getByRole("button", { name: "Open create menu" }).click();
  await page.getByRole("menuitem", { name: "Import an ICS file" }).click();
  const dialog = page.getByRole("dialog", { name: "Import an ICS file" });
  const title = "ページが見つかりません";
  await dialog.getByLabel("ICS file", { exact: true }).setInputFiles({
    name: "events.ics",
    mimeType: "text/calendar",
    buffer: Buffer.from(
      `BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:language-315\r\nDTSTAMP:20261001T000000Z\r\nDTSTART;TZID=Asia/Tokyo:20261010T200000\r\nDTEND;TZID=Asia/Tokyo:20261010T210000\r\nSUMMARY:${title}\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n`,
    ),
  });
  await expect(dialog).toContainText("1 of 1 selected");
  await expect(dialog).toContainText("Oct 10, 2026");
  await noOverflow(page);
  let releaseImport!: () => void;
  const importReady = new Promise<void>((resolve) => {
    releaseImport = resolve;
  });
  await page.route(`**/events/${guild}/bulk`, async (route) => {
    await importReady;
    await route.continue();
  });
  const response = page.waitForResponse(
    (r) => r.url().endsWith("/bulk") && r.request().method() === "POST",
  );
  await dialog
    .getByRole("button", { name: "Import 1 event", exact: true })
    .click();
  try {
    await expect(
      dialog.getByRole("button", { name: "Importing…", exact: true }),
    ).toBeDisabled();
  } finally {
    releaseImport();
  }
  const imported = await response;
  expect(imported.ok()).toBe(true);
  const [event] = await imported.json();
  expect(event.name).toBe(title);
  expect(event.start_at).toBe("2026-10-10T20:00:00");
  await expect(dialog).toContainText("Imported 1 event.");
  const share = await page.request.post(
    `/local/api/events/${guild}/${event.id}/share`,
  );
  expect(share.ok()).toBe(true);
  const { token } = await share.json();
  const anonymous = await browser.newContext({
    baseURL,
    storageState: { cookies: [], origins: [] },
    viewport: { width: 390, height: 844 },
  });
  const sharedPage = await anonymous.newPage();
  await english(sharedPage);
  try {
    await sharedPage.goto(`/share/${token}`);
    await expect(sharedPage).toHaveTitle(
      "ページが見つかりません | DisCalendar",
    );
    await expect(
      sharedPage.getByRole("heading", { name: title, exact: true }),
    ).toBeVisible();
    await expect(sharedPage.locator("article")).toContainText("Oct 10, 2026");
    await expect(sharedPage.locator("article")).toContainText("Japan time");
    await sharedPage
      .getByRole("combobox", { name: "Language / 言語" })
      .selectOption("ja");
    await expect(sharedPage.locator("article")).toContainText("日本時間");
    await expect(
      sharedPage.getByRole("heading", { name: title, exact: true }),
    ).toBeVisible();
    await noOverflow(sharedPage);
  } finally {
    await anonymous.close();
    expect(
      (
        await page.request.delete(`/local/api/events/${guild}/${event.id}`)
      ).ok(),
    ).toBe(true);
  }
});

test("英語の日程調整をキーボードで作成・投票し、候補日時を保持する", async ({
  page,
}) => {
  await english(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`/dashboard/${E2E_GUILDS.polls.id}/polls`);
  await page
    .getByRole("button", { name: "Create scheduling poll", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "Create scheduling poll" });
  const title = `日本語の候補 ${Date.now().toString(36)}`;
  await dialog.getByLabel("Title", { exact: true }).fill(title);
  await dialog.getByLabel("Option 1 start time").fill("20:30");
  await dialog.getByLabel("Option 1 end time").fill("21:30");
  const created = page.waitForResponse(
    (r) =>
      r.url().endsWith(`/polls/${E2E_GUILDS.polls.id}`) &&
      r.request().method() === "POST",
  );
  const save = dialog.getByRole("button", {
    name: "Create scheduling poll",
    exact: true,
  });
  await save.focus();
  await save.press("Enter");
  const result = await (await created).json();
  expect(result.poll.title).toBe(title);
  expect(result.poll.options[0].start_at).toMatch(/T20:30:00$/);
  expect(result.poll.options[0].end_at).toMatch(/T21:30:00$/);
  await expect(page.getByRole("heading", { name: title })).toBeVisible();
  const yes = page
    .getByRole("button", { name: "○ Available", exact: true })
    .first();
  await yes.focus();
  await yes.press("Enter");
  await expect(yes).toHaveAttribute("aria-pressed", "true");
  await expect(page.getByText("1 person has voted.")).toBeVisible();
  await noOverflow(page);
  await page.screenshot({
    path: test.info().outputPath("poll-390.png"),
    fullPage: true,
  });
  expect(
    (
      await page.request.delete(
        `/local/api/polls/${E2E_GUILDS.polls.id}/${result.poll.id}`,
      )
    ).ok(),
  ).toBe(true);
});

test("英語のプッシュ設定とMCP同意・エラーを390pxで操作できる", async ({
  page,
  context,
}) => {
  await english(page);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`/dashboard/${guild}`);
  await page.getByRole("button", { name: "Account menu" }).click();
  await page.getByRole("menuitem", { name: "Push notifications" }).click();
  const push = page.getByRole("dialog", { name: "Push notifications" });
  await expect(push).toHaveAttribute("lang", "en");
  await expect(
    push.getByRole("radio", { name: "Off", exact: true }),
  ).toBeVisible();
  await expect(
    push.getByText("Events to notify about", { exact: true }),
  ).toBeVisible();
  await noOverflow(page);
  await page.keyboard.press("Escape");
  if (process.env.MCP_ENABLED === "true") {
    await page.goto(
      "/mcp/consent?client_id=https%3A%2F%2Fclient.example%2Fmetadata.json&scope=guilds%3Aread%20events%3Aread&state=unchanged",
    );
    await expect(
      page.getByRole("heading", { name: "Authorize MCP connection" }),
    ).toBeVisible();
    const scopes = page.getByRole("group", { name: "Operations to allow" });
    await expect(
      scopes.getByRole("checkbox", { name: "Read events" }),
    ).toBeChecked();
    const deny = page.getByRole("button", { name: "Deny", exact: true });
    await deny.focus();
    await expect(deny).toBeFocused();
    await page.route("**/mcp/consent/submit", async (route) => {
      expect(route.request().postData()).toContain("unchanged");
      await route.fulfill({ status: 400, json: {} });
    });
    await deny.press("Enter");
    await expect(page.getByRole("main").getByRole("alert")).toContainText(
      "Could not complete the operation",
    );
    await noOverflow(page);
    await page.screenshot({
      path: test.info().outputPath("mcp-consent-390.png"),
      fullPage: true,
    });
  }
  await context.clearCookies();
  await page.goto("/mcp/login?state=unchanged");
  await page.route("**/mcp/login/start", (route) =>
    route.fulfill({ status: 400, json: {} }),
  );
  await page.getByRole("button", { name: "Sign in with Discord" }).click();
  await expect(page.getByRole("main").getByRole("alert")).toContainText(
    "Could not start sign-in",
  );
});

test.describe("海外のブラウザでもJSTを維持する", () => {
  test.use({ timezoneId: "America/New_York" });
  test("外部カレンダーの取得日時は保存済みの日本時間で表示する", async ({
    page,
  }) => {
    await english(page);
    await page.route(`**/guilds/${guild}/external-calendars`, (route) =>
      route.fulfill({
        json: [
          {
            id: 1,
            guild_id: guild,
            name: "海外表示",
            color: "#F44336",
            url: "https://example.com/calendar.ics",
            created_by: "100000000000000001",
            created_at: "2026-10-03T20:30:00",
            last_fetched_at: "2026-10-03T20:30:00",
            last_error: null,
          },
        ],
      }),
    );
    await page.goto(`/dashboard/${guild}`);
    await page
      .getByRole("button", { name: "Server settings", exact: true })
      .click();
    const external = page
      .getByRole("dialog")
      .locator("section[aria-labelledby=external-calendar-heading]");
    await expect(external).toContainText("Oct 3, 2026, 8:30 PM (Japan time)");
  });
});
