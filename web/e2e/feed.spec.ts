import { expect, type Page, test } from "@playwright/test";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS } from "./fixtures";
import {
  deleteEventsNamed,
  deleteFeedToken,
  insertEvent,
  setFeedToken,
} from "./seed";

// iCal フィード (#95)。サーバー設定ダイアログからの発行・再発行・無効化と、認証なしの配信 (/feeds/<token>.ics)。
// admin ギルドではテストユーザーがオーナー、member ギルドでは権限のない一般メンバー (Discord モックの定義)

/** 発行された URL の形 (web のオリジン + /feeds/<64 文字の 16 進>.ics) */
const FEED_URL = /\/feeds\/[0-9a-f]{64}\.ics$/;

// 他のテストと DB を共有するので、実行ごとに違う名前にして後片付けする
const stamp = Date.now().toString(36);
const feedTitle = `E2E フィード予定 ${stamp}`;

async function openSettings(page: Page) {
  await page.getByRole("button", { name: "サーバー設定" }).click();
  const dialog = page.getByRole("dialog", { name: "サーバー設定" });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** 折り返し (CRLF + 空白) を戻して 1 行ずつにする */
function unfold(ics: string): string[] {
  return ics.replace(/\r\n[ \t]/g, "").split("\r\n");
}

test.describe("管理権限のあるギルド", () => {
  const guildId = E2E_GUILDS.admin.id;

  test.beforeAll(async () => {
    await insertEvent(DATABASE_URL, guildId, {
      name: feedTitle,
      start_at: "2026-10-01 10:00:00",
      end_at: "2026-10-01 11:00:00",
    });
  });

  test.afterAll(async () => {
    await deleteEventsNamed(DATABASE_URL, [feedTitle]);
    await deleteFeedToken(DATABASE_URL, guildId);
  });

  test("発行した URL で予定を取得でき、再発行で古い URL は使えなくなり、無効化で止まる", async ({
    page,
    playwright,
  }) => {
    // 前のテストや retry の状態に依存しないよう未発行から始める
    await deleteFeedToken(DATABASE_URL, guildId);
    // 外部カレンダーと同じくログインなしで取りに行く (ブラウザの cookie を持たないコンテキスト)
    const anonymous = await playwright.request.newContext();

    await page.goto(`/dashboard/${guildId}`);
    const dialog = await openSettings(page);
    await expect(
      dialog.getByText("まだ発行されていません", { exact: false }),
    ).toHaveCount(0);
    await dialog.getByRole("button", { name: "フィード URL を発行" }).click();

    const urlInput = dialog.getByRole("textbox", { name: "フィード URL" });
    await expect(urlInput).toBeVisible();
    const url = await urlInput.inputValue();
    expect(url).toMatch(FEED_URL);
    expect(url.startsWith(page.url().split("/dashboard")[0])).toBe(true);

    // 配信: text/calendar、共有キャッシュに乗せない、seed した予定とサーバー名が入っている
    const res = await anonymous.get(url);
    expect(res.status()).toBe(200);
    expect(res.headers()["content-type"]).toContain("text/calendar");
    expect(res.headers()["cache-control"]).toContain("private");
    const lines = unfold(await res.text());
    expect(lines[0]).toBe("BEGIN:VCALENDAR");
    expect(lines).toContain(`X-WR-CALNAME:${E2E_GUILDS.admin.name}`);
    expect(lines).toContain(`SUMMARY:${feedTitle}`);
    expect(lines).toContain("DTSTART;TZID=Asia/Tokyo:20261001T100000");
    expect(lines).toContain("DTEND;TZID=Asia/Tokyo:20261001T110000");

    // 再発行 (確認ダイアログを挟む)。URL が変わり、古い URL は 404 になる
    await dialog.getByRole("button", { name: "再発行" }).click();
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "再発行" })
      .click();
    await expect(urlInput).not.toHaveValue(url);
    const reissued = await urlInput.inputValue();
    expect(reissued).toMatch(FEED_URL);
    expect((await anonymous.get(url)).status()).toBe(404);
    expect((await anonymous.get(reissued)).status()).toBe(200);

    // 無効化。発行ボタンに戻り、新しい URL も 404 になる
    await dialog.getByRole("button", { name: "無効化" }).click();
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "無効化" })
      .click();
    await expect(
      dialog.getByRole("button", { name: "フィード URL を発行" }),
    ).toBeVisible();
    expect((await anonymous.get(reissued)).status()).toBe(404);

    // 再読込しても未発行のまま (キャッシュだけの変更ではない)
    await dialog.getByRole("button", { name: "キャンセル" }).click();
    await page.reload();
    const reopened = await openSettings(page);
    await expect(
      reopened.getByRole("button", { name: "フィード URL を発行" }),
    ).toBeVisible();

    await anonymous.dispose();
  });

  test("形の合わないトークンや未発行のトークンは 404", async ({
    playwright,
    baseURL,
  }) => {
    const anonymous = await playwright.request.newContext({ baseURL });
    expect((await anonymous.get("/feeds/not-a-token.ics")).status()).toBe(404);
    expect((await anonymous.get(`/feeds/${"0".repeat(64)}.ics`)).status()).toBe(
      404,
    );
    await anonymous.dispose();
  });
});

test.describe("管理権限のないギルド", () => {
  const guildId = E2E_GUILDS.member.id;

  test.afterAll(async () => {
    await deleteFeedToken(DATABASE_URL, guildId);
  });

  test("未発行なら案内だけで、発行は API 側でも拒否される", async ({
    page,
  }) => {
    await deleteFeedToken(DATABASE_URL, guildId);
    await page.goto(`/dashboard/${guildId}`);
    const dialog = await openSettings(page);
    await expect(
      dialog.getByText("まだ発行されていません", { exact: false }),
    ).toBeVisible();
    await expect(
      dialog.getByRole("button", { name: "フィード URL を発行" }),
    ).toHaveCount(0);

    // 表示だけの制御ではない (ブラウザと同じ cookie で直接 API を叩く)
    const issue = await page.request.post(`/local/api/guilds/${guildId}/feed`);
    expect(issue.status()).toBe(403);
    expect(await issue.json()).toMatchObject({ error: "forbidden" });
    const revoke = await page.request.delete(
      `/local/api/guilds/${guildId}/feed`,
    );
    expect(revoke.status()).toBe(403);
  });

  test("発行済みなら URL を見てコピーできるが、再発行・無効化はできない", async ({
    page,
  }) => {
    // 一般メンバーは発行できないので、管理者が発行した状態を DB に直接作る
    const token = "e2e0".repeat(16);
    await setFeedToken(DATABASE_URL, guildId, token);
    await page.goto(`/dashboard/${guildId}`);
    const dialog = await openSettings(page);
    const urlInput = dialog.getByRole("textbox", { name: "フィード URL" });
    await expect(urlInput).toHaveValue(new RegExp(`/feeds/${token}\\.ics$`));
    await expect(dialog.getByRole("button", { name: "コピー" })).toBeVisible();
    await expect(dialog.getByRole("button", { name: "再発行" })).toHaveCount(0);
    await expect(dialog.getByRole("button", { name: "無効化" })).toHaveCount(0);
  });
});
