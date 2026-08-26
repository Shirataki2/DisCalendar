import { expect, type Page, test } from "@playwright/test";
import { setBotJoined } from "./discord-mock";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS } from "./fixtures";
import { addGuild, removeGuild } from "./seed";

// Bot の招待。Discord の Bot 追加画面は別タブで開くので、この画面 (Server Component) に
// 戻ってきたときに参加状況を問い合わせ、参加済みならそのサーバーのカレンダーへ移動する

const GUILD = E2E_GUILDS.invitable;

/** サーバー選択を開く。Discord の Bot 追加画面 (discord.com) には出ない (別タブは空のまま開く) */
async function openDashboard(page: Page) {
  await page
    .context()
    .route("https://discord.com/**", (route) => route.abort());
  await page.goto("/dashboard");
}

/** 招待リンクを開き、開いた別タブを返す */
async function openInvite(page: Page) {
  await openDashboard(page);
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

/** 参加状況の問い合わせ (1 回分) を待つ */
function waitForJoinedCheck(page: Page) {
  return page.waitForResponse((res) =>
    res.url().includes("/local/api/guilds/joined"),
  );
}

/** 別タブで Bot の追加が済んだ状態にする (bot/ が guilds テーブルに行を入れ、Discord 側も参加済みになる) */
async function completeInvite() {
  await addGuild(DATABASE_URL, GUILD);
  await setBotJoined(GUILD.id, true);
}

test.describe("Bot の招待", () => {
  test.afterEach(async () => {
    // 招待で参加した状態を元に戻す (他のテストと DB・Discord モックを共有している)
    await removeGuild(DATABASE_URL, GUILD.id);
    await setBotJoined(GUILD.id, false);
  });

  test("招待を終えて戻るとそのサーバーのカレンダーが開く", async ({ page }) => {
    const popup = await openInvite(page);
    await completeInvite();

    await popup.close();
    await returnFromInvite(page);

    await expect(page).toHaveURL(`/dashboard/${GUILD.id}`);
    await expect(page.getByRole("grid")).toBeVisible();
  });

  test("中クリック (新しいタブで開く) で招待した場合も移動する", async ({
    page,
  }) => {
    await openDashboard(page);
    // 中クリックは click ではなく auxclick になる (Playwright の中クリックでは別タブは開かないが、
    // ページに auxclick は届くので、招待先として覚えられるかを確かめられる)
    await page
      .getByRole("link", { name: new RegExp(GUILD.name) })
      .click({ button: "middle" });
    await completeInvite();

    await returnFromInvite(page);

    await expect(page).toHaveURL(`/dashboard/${GUILD.id}`);
  });

  test("Bot の参加が届くのが遅れても、試し直して移動する", async ({ page }) => {
    const popup = await openInvite(page);

    // 戻った時点では Bot がまだ guilds テーブルに書けていない (1 回目の問い合わせは空振り)
    const firstCheck = waitForJoinedCheck(page);
    await popup.close();
    await returnFromInvite(page);
    await firstCheck;
    await expect(page).toHaveURL("/dashboard");

    await completeInvite();

    // 少し間を置いた試し直しで参加を検知する
    await expect(page).toHaveURL(`/dashboard/${GUILD.id}`);
  });

  test("招待をやめて戻ってきたときはサーバー選択のまま", async ({ page }) => {
    const popup = await openInvite(page);

    const check = waitForJoinedCheck(page);
    await popup.close();
    await returnFromInvite(page);

    // 参加状況の問い合わせは飛ぶが、参加していないので移動しない
    await check;
    await expect(page).toHaveURL("/dashboard");
    await expect(
      page.getByRole("heading", { name: "Bot を招待できるサーバー" }),
    ).toBeVisible();
  });
});
