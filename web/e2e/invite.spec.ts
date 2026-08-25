import { expect, type Page, test } from "@playwright/test";
import { setBotJoined } from "./discord-mock";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS } from "./fixtures";
import { addGuild, removeGuild } from "./seed";

// Bot の招待。Discord の Bot 追加画面は別タブで開くので、この画面 (Server Component) に
// 戻ってきたときに参加状況を問い合わせ、参加済みならそのサーバーのカレンダーへ移動する

const GUILD = E2E_GUILDS.invitable;

/** サーバー選択を開き、招待リンクをクリックして開いた別タブを返す */
async function openInvite(page: Page) {
  // Discord の Bot 追加画面 (discord.com) には出ない。別タブは空のまま開く
  await page
    .context()
    .route("https://discord.com/**", (route) => route.abort());
  await page.goto("/dashboard");
  const opened = page.waitForEvent("popup");
  await page.getByRole("link", { name: new RegExp(GUILD.name) }).click();
  return await opened;
}

/**
 * 招待のタブから戻ってきたことをサーバー選択のタブに伝える。
 * Playwright は headless / headed のどちらでもタブを前面にしただけでは visibilitychange も
 * focus も起こさない (document.visibilityState は常に "visible") ので、戻ってきた合図だけを手で送る
 */
async function returnFromInvite(page: Page) {
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
}

test.describe("Bot の招待", () => {
  test.afterEach(async () => {
    // 招待で参加した状態を元に戻す (他のテストと DB・Discord モックを共有している)
    await removeGuild(DATABASE_URL, GUILD.id);
    await setBotJoined(GUILD.id, false);
  });

  test("招待を終えて戻るとそのサーバーのカレンダーが開く", async ({ page }) => {
    const popup = await openInvite(page);

    // 別タブで Bot の追加が済んだ状態にする (bot/ が guilds テーブルに行を入れ、Discord 側も参加済みになる)
    await addGuild(DATABASE_URL, GUILD);
    await setBotJoined(GUILD.id, true);

    await popup.close();
    await returnFromInvite(page);

    await expect(page).toHaveURL(`/dashboard/${GUILD.id}`);
    await expect(page.getByRole("grid")).toBeVisible();
  });

  test("招待をやめて戻ってきたときはサーバー選択のまま", async ({ page }) => {
    const popup = await openInvite(page);

    const joined = page.waitForResponse((res) =>
      res.url().includes("/local/api/guilds/joined"),
    );
    await popup.close();
    await returnFromInvite(page);

    // 参加状況の問い合わせは飛ぶが、参加していないので移動しない
    await joined;
    await expect(page).toHaveURL("/dashboard");
    await expect(
      page.getByRole("heading", { name: "Bot を招待できるサーバー" }),
    ).toBeVisible();
  });
});
