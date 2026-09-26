import { expect, type Locator, test } from "@playwright/test";
import { openEventPopover } from "./calendar";
import { WEB_URL } from "./env";
import { E2E_GUILDS } from "./fixtures";

const guild = E2E_GUILDS.admin.id;
const description = [
  "# 募集要項",
  "__**重要**__ *斜体* ~~取り消し~~",
  "- 持ち物",
  "- ルール",
  "",
  "1. 集合",
  "2. 開始",
  "",
  "> 引用",
  "`コード`",
  ["```", `長いコード${"a".repeat(100)}`, "```"].join("\n"),
  "[案内](https://example.com/guide)",
  "||秘密の合言葉||",
  '<img src=x onerror="alert(1)">',
  "[危険](javascript:alert%281%29)",
].join("\n");

async function checkDescription(container: Locator) {
  const rendered = container.locator('[data-slot="event-description"]');
  await expect(
    rendered.getByRole("heading", { name: "募集要項" }),
  ).toBeVisible();
  await expect(rendered.locator("u strong")).toHaveText("重要");
  await expect(rendered.locator("ul li")).toHaveCount(2);
  await expect(rendered.locator("ol li")).toHaveCount(2);
  await expect(rendered.getByRole("link", { name: "案内" })).toHaveAttribute(
    "rel",
    "noopener noreferrer nofollow",
  );
  await expect(rendered.getByRole("link", { name: "危険" })).toHaveCount(0);
  await expect(rendered.locator("img,script,iframe,svg")).toHaveCount(0);
  const spoiler = rendered.locator("details");
  await expect(spoiler.getByText("秘密の合言葉")).toBeHidden();
  await spoiler.locator("summary").focus();
  await spoiler.locator("summary").press("Enter");
  await expect(spoiler.getByText("秘密の合言葉")).toBeVisible();
  await spoiler.locator("summary").press("Space");
  await expect(spoiler.getByText("秘密の合言葉")).toBeHidden();
  expect(
    await rendered.evaluate((el) => el.scrollWidth <= el.clientWidth + 1),
  ).toBe(true);
}

for (const width of [1280, 390]) {
  test(`説明のプレビュー・保存・編集・横断表示・匿名共有 (${width}px)`, async ({
    page,
    browser,
  }, testInfo) => {
    await page.setViewportSize({ width, height: 900 });
    const noon = new Date();
    noon.setUTCHours(3, 0, 0, 0);
    await page.clock.setFixedTime(noon);
    await page.goto(`/dashboard/${guild}`);
    await page.getByRole("button", { name: "新規作成" }).click();
    const dialog = page.getByRole("dialog", { name: "予定を作成" });
    const name = `書式 ${width} ${Date.now().toString(36)}`;
    await dialog.getByLabel("タイトル").fill(name);
    await dialog.getByLabel("説明", { exact: true }).fill(description);
    const preview = dialog.getByRole("button", {
      name: "プレビュー",
      exact: true,
    });
    await preview.focus();
    await preview.press("Space");
    await expect(preview).toHaveAttribute("aria-pressed", "true");
    await checkDescription(dialog);
    await page.screenshot({
      path: testInfo.outputPath(`preview-${width}.png`),
    });
    await preview.click();
    await expect(dialog.getByLabel("説明", { exact: true })).toHaveValue(
      description,
    );
    await preview.click();
    const saved = page.waitForResponse(
      (res) =>
        res.url().endsWith(`/local/api/events/${guild}`) &&
        res.request().method() === "POST",
    );
    await dialog.getByRole("button", { name: "作成", exact: true }).click();
    const response = await saved;
    expect(response.status()).toBe(201);
    const event = await response.json();
    expect(event.description).toBe(description);
    try {
      await expect(dialog).toBeHidden();
      const popover = await openEventPopover(page, name);
      await checkDescription(popover);
      await popover.getByRole("button", { name: "編集", exact: true }).click();
      const edit = page.getByRole("dialog", { name: "予定を編集" });
      await expect(edit.getByLabel("説明", { exact: true })).toHaveValue(
        description,
      );
      await edit
        .getByLabel("説明", { exact: true })
        .fill(`**${"あ".repeat(997)}**`);
      await expect(edit.getByText("1001/1000", { exact: true })).toBeVisible();
      await edit
        .getByRole("button", { name: "プレビュー", exact: true })
        .click();
      await edit.getByRole("button", { name: "保存", exact: true }).click();
      await expect(edit.getByLabel("説明", { exact: true })).toBeFocused();
      await expect(
        edit.getByText("説明は1000文字以内で入力してください"),
      ).toBeVisible();
      await edit.getByLabel("説明", { exact: true }).fill(description);
      await edit.getByRole("button", { name: "保存", exact: true }).click();
      await expect(edit).toBeHidden();
      await page.goto("/dashboard/all");
      await checkDescription(await openEventPopover(page, name));
      const issued = await page.request.post(
        `/local/api/events/${guild}/${event.id}/share`,
      );
      expect(issued.ok()).toBe(true);
      const { token } = await issued.json();
      const anonymous = await browser.newContext({
        storageState: { cookies: [], origins: [] },
        viewport: { width, height: 900 },
      });
      try {
        const shared = await anonymous.newPage();
        await shared.goto(`${WEB_URL}/share/${token}`);
        await checkDescription(shared.locator("article"));
        await shared.screenshot({
          path: testInfo.outputPath(`share-${width}.png`),
          fullPage: true,
        });
        expect(
          await shared.evaluate(
            () => document.documentElement.scrollWidth <= window.innerWidth,
          ),
        ).toBe(true);
      } finally {
        await anonymous.close();
      }
    } finally {
      await page.request.delete(`/local/api/events/${guild}/${event.id}`);
    }
  });
}
