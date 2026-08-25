import path from "node:path";
import { expect, type Locator, type Page, test } from "@playwright/test";
import { calendarToday, eventOn } from "../calendar";
import { WEB_DIR } from "../env";
import { E2E_GUILDS } from "../fixtures";
import {
  CREATE_SAMPLE,
  EDIT_TARGET,
  POPOVER_TARGET,
  sampleEvents,
} from "./sample-data";

// LP (src/assets/lp/) と使い方 (src/assets/docs/) のスクリーンショットを撮り直す。
// 通常の pnpm e2e では動かない (playwright.config.ts の testIgnore)。実行は web/ で:
//
//   pnpm shot
//
// 手順とチェック観点は .claude/skills/update-screenshots/SKILL.md を見る。
// ここは「アプリが壊れていないこと」ではなく「画像が作られること」を確認するテストなので、
// 失敗したときは画像が更新されない = 撮り直しが必要、という意味になる

const guildId = E2E_GUILDS.admin.id;

// 同じ DB を順に使う (サンプルの予定を最初に 1 回だけ入れる)
test.describe.configure({ mode: "serial" });

test.describe("カレンダー", () => {
  // LP のヒーロー画像は横長で使うので、月表示が 1 画面に収まる比率にする
  test.use({ viewport: { width: 1400, height: 760 }, deviceScaleFactor: 2 });

  test("サンプルの予定を入れる", async ({ page }) => {
    await page.goto(`/dashboard/${guildId}`);
    const today = await calendarToday(page);
    for (const event of sampleEvents(today)) {
      const res = await page.request.post(`/local/api/events/${guildId}`, {
        data: event,
      });
      expect(res.status(), await res.text()).toBe(201);
    }
  });

  test("lp/calendar.png (カレンダー画面)", async ({ page }) => {
    await openCalendar(page);
    await page.screenshot({ path: assetPath("lp/calendar.png") });
  });

  test("docs/popover.png (予定のポップオーバー)", async ({ page }) => {
    await openCalendar(page);
    await eventOn(page, POPOVER_TARGET).click();
    const popover = page
      .getByRole("dialog")
      .filter({ hasText: POPOVER_TARGET });
    await expect(popover).toBeVisible();
    await settle(page);
    await page.screenshot({
      path: assetPath("docs/popover.png"),
      clip: await clipAround(page, [popover], { x: 170, y: 30 }),
    });
  });

  test("docs/serverselect.png (サーバー選択)", async ({ page }) => {
    await page.goto("/dashboard");
    await expect(
      page.getByRole("heading", { name: "サーバーを選択" }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Bot を招待できるサーバー" }),
    ).toBeVisible();
    await prepare(page);
    await settle(page);
    // 下は空白なので、招待できるサーバーの行までで切る
    await page.screenshot({
      path: assetPath("docs/serverselect.png"),
      clip: { x: 0, y: 0, width: 1400, height: 430 },
    });
  });
});

test.describe("ダイアログ", () => {
  // 予定のフォームは縦に長いので、スクロールバーが出ない高さにする
  test.use({ viewport: { width: 1400, height: 1000 }, deviceScaleFactor: 2 });

  test("lp/dialog.png (予定の編集)", async ({ page }) => {
    await openCalendar(page);
    await eventOn(page, EDIT_TARGET).click();
    const popover = page.getByRole("dialog").filter({ hasText: EDIT_TARGET });
    await popover.getByRole("button", { name: "編集" }).click();

    const dialog = page.getByRole("dialog", { name: "予定を編集" });
    await expect(dialog.getByLabel("タイトル")).toHaveValue(EDIT_TARGET);
    await blur(dialog);
    await settle(page);
    await page.screenshot({
      path: assetPath("lp/dialog.png"),
      clip: await clipAround(page, [dialog], { x: 48, y: 40 }),
    });
  });

  test("docs/create-dialog.png (予定の作成)", async ({ page }) => {
    await openCalendar(page);
    await page.getByRole("button", { name: "新規作成" }).click();

    const dialog = page.getByRole("dialog", { name: "予定を作成" });
    await expect(dialog).toBeVisible();
    await dialog.getByLabel("タイトル").fill(CREATE_SAMPLE.name);
    await dialog.getByLabel("開始時刻").fill(CREATE_SAMPLE.startTime);
    await dialog.getByLabel("終了時刻").fill(CREATE_SAMPLE.endTime);
    await dialog.getByLabel("説明").fill(CREATE_SAMPLE.description);
    await blur(dialog);
    await settle(page);
    // 保存はしない (カレンダーの画像に写らないようにするため)
    await page.screenshot({
      path: assetPath("docs/create-dialog.png"),
      clip: await clipAround(page, [dialog], { x: 48, y: 40 }),
    });
  });

  test("lp/settings.png (サーバー設定)", async ({ page }) => {
    await openCalendar(page);
    await page.getByRole("button", { name: "サーバー設定" }).click();

    const dialog = page.getByRole("dialog", { name: "サーバー設定" });
    await expect(dialog).toBeVisible();
    // 「限定する」を選んだ状態を見せる (保存はしないので DB は変わらない)
    await dialog.getByRole("checkbox").check();
    await blur(dialog);
    await settle(page);
    await page.screenshot({
      path: assetPath("lp/settings.png"),
      clip: await clipAround(page, [dialog], { x: 48, y: 40 }),
    });
  });
});

test.describe("ログイン", () => {
  test.use({
    viewport: { width: 1000, height: 700 },
    deviceScaleFactor: 2,
    // ログイン前の画面なので、共有のセッション cookie を持たせない
    storageState: { cookies: [], origins: [] },
  });

  test("docs/login.png (ログインページ)", async ({ page }) => {
    await page.goto("/login");
    const heading = page.getByRole("heading", { name: "ログイン" });
    const button = page.getByRole("button", { name: "Discordでログイン" });
    // 末尾の但し書きまで入れて切る (途中で切れないように)
    const notice = page.locator("main p").last();
    await expect(button).toBeVisible();
    await prepare(page);
    await settle(page);
    await page.screenshot({
      path: assetPath("docs/login.png"),
      clip: await clipAround(page, [heading, button, notice], {
        x: 400,
        y: 40,
      }),
    });
  });
});

/** 差し替える画像の場所 (LP と使い方が import しているファイルをそのまま上書きする) */
function assetPath(name: string): string {
  return path.join(WEB_DIR, "src/assets", name);
}

/** サンプルの予定が並んだカレンダーを開く */
async function openCalendar(page: Page) {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  // 月内に必ずある予定 (13〜15 日と 21 日) が出るまで待つ
  await expect(eventOn(page, "合宿")).toBeVisible();
  await expect(eventOn(page, "企画会議")).toBeVisible();
  await prepare(page);
}

/**
 * 撮影用に画面を整える:
 * - next dev の開発ツール (左下のロゴ) を消す
 * - フォーカスリングを消す (Playwright のクリックで付くが、実際のマウス操作では出ない)
 * - ロゴのフォント (preload していない) の読み込みを待つ
 */
async function prepare(page: Page) {
  await page.addStyleTag({
    content:
      "nextjs-portal { display: none !important; }" +
      " *:focus-visible { outline: none !important; box-shadow: none !important; }",
  });
  await page.evaluate(() => document.fonts.ready);
}

/** 入力欄からフォーカスを外す (カーソルが写らないようにする) */
async function blur(dialog: Locator) {
  await dialog.getByRole("heading").first().click();
}

/** 開くアニメーションと予定の描画が落ち着くのを待つ */
async function settle(page: Page) {
  await page.waitForTimeout(800);
}

/** 対象の要素をすべて含む範囲を、余白を付けて切り出す (画面からはみ出さないように丸める) */
async function clipAround(
  page: Page,
  targets: Locator[],
  pad: { x: number; y: number },
) {
  const viewport = page.viewportSize();
  if (!viewport) throw new Error("viewport のサイズが取れません");

  const boxes = await Promise.all(targets.map((t) => t.boundingBox()));
  const found = boxes.filter((box) => box !== null);
  if (found.length !== targets.length) {
    throw new Error("撮影する要素が表示されていません");
  }
  const left = Math.min(...found.map((b) => b.x));
  const top = Math.min(...found.map((b) => b.y));
  const right = Math.max(...found.map((b) => b.x + b.width));
  const bottom = Math.max(...found.map((b) => b.y + b.height));

  const x = Math.max(0, left - pad.x);
  const y = Math.max(0, top - pad.y);
  return {
    x,
    y,
    width: Math.min(viewport.width - x, right - x + pad.x),
    height: Math.min(viewport.height - y, bottom - y + pad.y),
  };
}
