import { writeFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { createEvent, openEventPopover } from "./calendar";
import { WEB_URL } from "./env";
import { E2E_GUILDS } from "./fixtures";

const guild = E2E_GUILDS.admin.id;
const input = {
  name: "共有予定のテスト",
  description: "ログイン不要で見られる説明",
  notifications: [],
  color: "#5865F2",
  is_all_day: false,
  start_at: "2026-09-05T10:00:00",
  end_at: "2026-09-05T11:00:00",
};

test("共有の発行・匿名閲覧・編集・失効・削除と認可", async ({
  page,
  playwright,
  browser,
}, testInfo) => {
  test.setTimeout(180_000);
  const anonymous = await playwright.request.newContext({
    baseURL: WEB_URL,
    storageState: { cookies: [], origins: [] },
  });
  const created = await page.request.post(`/local/api/events/${guild}`, {
    data: input,
  });
  expect(created.status()).toBe(201);
  const event = await created.json();
  const path = `/local/api/events/${guild}/${event.id}/share`;
  try {
    expect((await anonymous.post(path)).status()).toBe(401);
    expect((await page.request.get(path)).status()).toBe(200);
    expect(await (await page.request.get(path)).json()).toBeNull();
    const issued = await page.request.post(path);
    expect(issued.status()).toBe(200);
    const { token } = await issued.json();
    expect(token).toMatch(/^[0-9a-f]{64}$/);
    expect((await (await page.request.post(path)).json()).token).toBe(token);
    const publicPath = `/share/${token}`;
    const publicApi = await anonymous.get(`/local/api${publicPath}`);
    expect(publicApi.status()).toBe(200);
    expect(Object.keys(await publicApi.json()).sort()).toEqual([
      "description",
      "end_at",
      "guild_avatar_url",
      "guild_id",
      "guild_name",
      "is_all_day",
      "name",
      "start_at",
    ]);
    const response = await anonymous.get(publicPath, {
      headers: { "User-Agent": "Twitterbot" },
    });
    expect(response.status()).toBe(200);
    const html = await response.text();
    expect(html).toContain(input.description);
    expect(html).toContain('name="robots" content="noindex, nofollow"');
    const imageUrl = html.match(/property="og:image" content="([^"]+)"/)?.[1];
    expect(imageUrl).toBeTruthy();
    expect(imageUrl).toMatch(/^https:\/\/discalendar\.app\/share\//);
    const imagePath = new URL(imageUrl as string).pathname;
    const image = await anonymous.get(imagePath);
    expect(image.status()).toBe(200);
    expect(image.headers()["content-type"]).toContain("image/png");
    expect(image.headers()["cache-control"]).toContain("no-store");
    await writeFile(testInfo.outputPath("opengraph.png"), await image.body());
    const browserContext = await browser.newContext({
      storageState: { cookies: [], origins: [] },
    });
    const publicPage = await browserContext.newPage();
    await publicPage.goto(`${WEB_URL}${publicPath}`);
    await expect(
      publicPage.getByRole("heading", { name: input.name }),
    ).toBeVisible();
    await publicPage.screenshot({
      path: testInfo.outputPath("share.png"),
      fullPage: true,
    });
    await browserContext.close();
    expect(
      (
        await page.request.put(`/local/api/events/${guild}/${event.id}`, {
          data: { ...input, name: "変更後の共有予定" },
        })
      ).status(),
    ).toBe(200);
    expect(await (await anonymous.get(publicPath)).text()).toContain(
      "変更後の共有予定",
    );
    expect((await page.request.delete(path)).status()).toBe(204);
    expect((await anonymous.get(`/local/api${publicPath}`)).status()).toBe(404);
    expect(
      (
        await anonymous.get(publicPath, {
          headers: { "User-Agent": "Twitterbot" },
        })
      ).status(),
    ).toBe(404);
    expect((await anonymous.get(imagePath)).status()).toBe(404);
    const next = await (await page.request.post(path)).json();
    expect(next.token).not.toBe(token);
    await page.request.delete(`/local/api/events/${guild}/${event.id}`);
    expect(
      (await anonymous.get(`/local/api/share/${next.token}`)).status(),
    ).toBe(404);
    // 別サーバーの予定 ID は発行・取得・失効のどれも通らない。
    for (const method of ["get", "post", "delete"] as const) {
      expect(
        (
          await page.request[method](
            `/local/api/events/${E2E_GUILDS.member.id}/${event.id}/share`,
          )
        ).status(),
      ).toBe(403);
      expect(
        (
          await page.request[method](
            `/local/api/events/${E2E_GUILDS.noEventsPerm.id}/${event.id}/share`,
          )
        ).status(),
      ).toBe(404);
    }
  } finally {
    await page.request.delete(`/local/api/events/${guild}/${event.id}`);
    await anonymous.dispose();
  }
});

test("編集画面からコピーと無効化ができる", async ({ page }) => {
  await page.goto(`/dashboard/${guild}`);
  const name = "E2E 共有操作";
  await createEvent(page, name);
  const popover = await openEventPopover(page, name);
  await popover.getByRole("button", { name: "編集", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "予定を編集" });
  await dialog
    .getByRole("button", { name: "共有リンクをコピー", exact: true })
    .click();
  const url = dialog.getByRole("textbox", { name: "共有リンク URL" });
  await expect(url).toHaveValue(/\/share\/[0-9a-f]{64}$/);
  const issued = await url.inputValue();
  await dialog
    .getByRole("button", { name: "共有リンクをコピー", exact: true })
    .click();
  await expect(url).toHaveValue(issued);
  await dialog
    .getByRole("button", { name: "共有リンクを無効化", exact: true })
    .click();
  await expect(url).toHaveCount(0);
  await expect(dialog.getByRole("status")).toContainText("無効化しました");
});
