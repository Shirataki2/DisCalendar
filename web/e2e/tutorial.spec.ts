import { expect, type Locator, type Page, test } from "@playwright/test";
import { Pool } from "pg";
import { createEvent, eventOn, openEventPopover } from "./calendar";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS, E2E_USER } from "./fixtures";

const guide = (page: Page) =>
  page.getByRole("region", { name: "チュートリアル", exact: true });

async function blockRealData(page: Page) {
  const requests: string[] = [];
  const realData = /\/local\/api\/|\/api\/auth\/|https:\/\/[^/]*discord[^/]*\//;
  // 本番の Service Worker を有効にしたまま、その通信も含めて監視する。
  page.context().on("request", (request) => {
    if (realData.test(request.url())) requests.push(request.url());
  });
  await page.context().route(realData, (route) => route.abort());
  return requests;
}

test.describe("公開チュートリアル", () => {
  test.use({
    storageState: { cookies: [], origins: [] },
  });

  test("未ログインで作成・編集・通知確認・削除を体験でき、実データにアクセスしない", async ({
    page,
  }) => {
    const requests = await blockRealData(page);
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto("/tutorial");
    await expect(page.getByRole("grid")).toBeVisible();
    await guide(page).getByRole("button", { name: "次へ" }).click();
    await expect(guide(page)).toContainText("ステップ 2 / 6");
    await expect(
      guide(page).getByRole("button", { name: "次へ" }),
    ).toBeDisabled();
    await page.getByRole("button", { name: "新規作成" }).click();
    const create = page.getByRole("dialog", { name: "予定を作成" });
    await create.getByRole("button", { name: "作成", exact: true }).click();
    await expect(create).toContainText("タイトルを入力してください");
    await expect(create).toContainText("ステップ 2 / 6");
    await create.getByLabel("タイトル").fill("練習のゲーム会");
    await create.getByRole("button", { name: "作成", exact: true }).click();
    await expect(create).toBeHidden();
    await expect(guide(page)).toContainText("ステップ 3 / 6");
    await expect(eventOn(page, "練習のゲーム会")).toBeVisible();

    const popover = await openEventPopover(page, "練習のゲーム会");
    await popover.getByRole("button", { name: "編集", exact: true }).click();
    const edit = page.getByRole("dialog", { name: "予定を編集" });
    await expect(edit.getByText("共有リンク", { exact: true })).toHaveCount(0);
    await expect(
      edit.getByText("Discord のイベントとしても作成する"),
    ).toHaveCount(0);
    await edit.getByLabel("タイトル").fill("練習のゲーム会・変更後");
    await edit.getByLabel("開始時刻").fill("14:00");
    await edit.getByLabel("終了時刻").fill("15:30");
    await edit
      .getByLabel("説明", { exact: true })
      .fill("ボイスチャンネルに集まろう");
    await edit.getByLabel("通知のタイミング (数値)").first().fill("2");
    await edit.getByRole("button", { name: "保存", exact: true }).click();
    await expect(edit).toBeHidden();
    await expect(guide(page)).toContainText("ステップ 4 / 6");
    await page.getByLabel("通知プレビュー", { exact: true }).selectOption("0");
    const preview = page.getByRole("figure", { name: "Discord通知の見本" });
    await expect(preview).toContainText("練習のゲーム会・変更後");
    await expect(preview).toContainText("ボイスチャンネルに集まろう");
    await expect(preview).toContainText("14:00 - 15:30");
    await expect(preview).toContainText("2日後に以下の予定が開催されます");
    await page
      .getByLabel("通知プレビュー", { exact: true })
      .selectOption("start");
    await expect(preview).toContainText("以下の予定が開催されます");
    await guide(page).getByRole("button", { name: "通知を確認した" }).click();

    const updated = await openEventPopover(page, "練習のゲーム会・変更後");
    await updated.getByRole("button", { name: "削除", exact: true }).click();
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "キャンセル" })
      .click();
    await expect(eventOn(page, "練習のゲーム会・変更後")).toBeVisible();
    await expect(page.getByRole("alertdialog")).toBeHidden();
    const reopened = await openEventPopover(page, "練習のゲーム会・変更後");
    await reopened.getByRole("button", { name: "削除", exact: true }).click();
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "削除", exact: true })
      .click();
    await expect(guide(page)).toContainText("ステップ 6 / 6");
    await expect(eventOn(page, "練習のゲーム会・変更後")).toHaveCount(0);
    await expect(
      guide(page).getByRole("link", { name: "Botを招待する" }),
    ).toHaveAttribute("href", "/invite");
    await guide(page).getByRole("button", { name: "前の説明へ" }).click();
    await guide(page).getByRole("button", { name: "次へ" }).click();
    await guide(page).getByRole("button", { name: "自由に試す" }).click();
    await createEvent(page, "自由に作った予定");
    await expect(guide(page)).toContainText("自由に試せます");
    expect(requests).toEqual([]);
    expect(errors).toEqual([]);
  });

  test("やり直し・再読込・再訪問でリセットし、個人設定を書き換えない", async ({
    page,
  }) => {
    await page.goto("/tutorial");
    await page.evaluate(() => {
      localStorage.setItem("discalendar-calendar-last-view", "timeGridWeek");
      localStorage.setItem(
        "discalendar-calendar-settings",
        JSON.stringify({ initialView: "last", defaultColor: "#009688" }),
      );
    });
    await page.reload();
    const settings = await page.evaluate(() => ({ ...localStorage }));
    await expect(
      page.getByRole("tab", { name: "月", exact: true }),
    ).toHaveAttribute("aria-selected", "true");
    await guide(page).getByRole("button", { name: "案内を終了" }).click();
    await createEvent(page, "リセットする予定");
    await page.getByRole("tab", { name: "リスト", exact: true }).click();
    expect(await page.evaluate(() => ({ ...localStorage }))).toEqual(settings);
    await page
      .getByRole("button", { name: "最初からやり直す", exact: true })
      .click();
    await expect(guide(page)).toContainText("ステップ 1 / 6");
    await expect(eventOn(page, "リセットする予定")).toHaveCount(0);
    await createEvent(page, "再読込する予定");
    await page.reload();
    await expect(eventOn(page, "再読込する予定")).toHaveCount(0);
    await createEvent(page, "再訪問する予定");
    await page.getByRole("link", { name: "使い方を見る" }).click();
    await page.getByRole("link", { name: "操作を試す", exact: true }).click();
    await expect(guide(page)).toContainText("ステップ 1 / 6");
    await expect(eventOn(page, "再訪問する予定")).toHaveCount(0);
    expect(await page.evaluate(() => ({ ...localStorage }))).toEqual(settings);
  });

  test("LP から自分で開始できる", async ({ page }) => {
    await page.goto("/");
    await page.getByRole("link", { name: "ログインせずに操作を試す" }).click();
    await expect(page).toHaveURL("/tutorial");
    await expect(guide(page)).toContainText("ステップ 1 / 6");
  });

  test("途中で予定を削除しても作り直して続けられる", async ({ page }) => {
    await page.goto("/tutorial");
    await guide(page).getByRole("button", { name: "次へ" }).click();
    await createEvent(page, "途中で消す予定");
    const popover = await openEventPopover(page, "途中で消す予定");
    await popover.getByRole("button", { name: "削除", exact: true }).click();
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "削除", exact: true })
      .click();
    await expect(guide(page)).toContainText("ステップ 2 / 6");
    await createEvent(page, "作り直した予定");
    await expect(guide(page)).toContainText("ステップ 3 / 6");
    await guide(page).getByRole("button", { name: "前の説明へ" }).click();
    await expect(eventOn(page, "作り直した予定")).toBeVisible();
    await guide(page).getByRole("button", { name: "次へ" }).click();
    await expect(guide(page)).toContainText("ステップ 3 / 6");
  });
});

for (const mode of ["タッチ", "キーボード"] as const) {
  test.describe(mode, () => {
    test.use({
      storageState: { cookies: [], origins: [] },
      ...(mode === "タッチ"
        ? {
            viewport: { width: 390, height: 844 },
            hasTouch: true,
            isMobile: true,
          }
        : {}),
    });
    test("マウスを使わずチュートリアルを完了できる", async ({ page }) => {
      const requests = await blockRealData(page);
      const activate = async (locator: Locator) => {
        if (mode === "タッチ") return locator.tap();
        for (let i = 0; i < 100; i++) {
          if (
            await locator.evaluate(
              (element) => element === document.activeElement,
            )
          )
            break;
          await page.keyboard.press("Tab");
        }
        await expect(locator).toBeFocused();
        await page.keyboard.press("Enter");
      };
      await page.goto("/tutorial");
      await activate(guide(page).getByRole("button", { name: "次へ" }));
      await activate(page.getByRole("button", { name: "新規作成" }));
      const create = page.getByRole("dialog", { name: "予定を作成" });
      await expect(create.getByLabel("タイトル")).toBeFocused();
      await page.keyboard.type("Keyboard game");
      await activate(create.getByRole("button", { name: "作成", exact: true }));
      await expect(guide(page)).toContainText("ステップ 3 / 6");
      await activate(eventOn(page, "Keyboard game"));
      await activate(
        page
          .getByRole("dialog")
          .getByRole("button", { name: "編集", exact: true }),
      );
      const edit = page.getByRole("dialog", { name: "予定を編集" });
      await expect(edit.getByLabel("タイトル")).toBeFocused();
      await page.keyboard.press("End");
      await page.keyboard.type(" updated");
      await activate(edit.getByRole("button", { name: "保存", exact: true }));
      await expect(guide(page)).toContainText("ステップ 4 / 6");
      await expect(
        page.getByRole("figure", { name: "Discord通知の見本" }),
      ).toBeVisible();
      await activate(
        guide(page).getByRole("button", { name: "通知を確認した" }),
      );
      await activate(eventOn(page, "Keyboard game updated"));
      await activate(
        page
          .getByRole("dialog")
          .getByRole("button", { name: "削除", exact: true }),
      );
      await activate(
        page
          .getByRole("alertdialog")
          .getByRole("button", { name: "削除", exact: true }),
      );
      await expect(guide(page)).toContainText("ステップ 6 / 6");
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
      expect(requests).toEqual([]);
    });
  });
}

test.describe("ログイン後の入口", () => {
  for (const state of ["no-guilds", "invitable-only"]) {
    test(`Bot 参加サーバー0件 (${state}) でも体験へ進める`, async ({
      page,
    }) => {
      const pool = new Pool({ connectionString: DATABASE_URL });
      try {
        await pool.query(
          'UPDATE "account" SET "accessToken" = $1 WHERE "userId" = $2',
          [`${E2E_USER.accessToken}-${state}`, E2E_USER.id],
        );
        await page.goto("/dashboard");
        await expect(
          page.getByText("Bot が参加しているサーバーがありません。", {
            exact: false,
          }),
        ).toBeVisible();
        if (state === "no-guilds")
          await expect(
            page.getByRole("heading", { name: "Bot を招待できるサーバー" }),
          ).toHaveCount(0);
        await page
          .getByRole("main")
          .getByRole("link", { name: /練習用カレンダーで操作を試す/ })
          .click();
        await expect(guide(page)).toContainText("ステップ 1 / 6");
      } finally {
        await pool.query(
          'UPDATE "account" SET "accessToken" = $1 WHERE "userId" = $2',
          [E2E_USER.accessToken, E2E_USER.id],
        );
        await pool.end();
      }
    });
  }

  test("編集権限がないサーバーから移動し、戻っても権限が変わらない", async ({
    page,
  }) => {
    const path = `/dashboard/${E2E_GUILDS.member.id}`;
    await page.goto(path);
    await expect(page.getByRole("button", { name: "新規作成" })).toBeDisabled();
    await page
      .getByRole("main")
      .getByRole("link", { name: "練習用カレンダーで操作を試す" })
      .click();
    await createEvent(page, "権限なしでも練習");
    await page.goto(path);
    await expect(page.getByRole("button", { name: "新規作成" })).toBeDisabled();
    await expect(eventOn(page, "権限なしでも練習")).toHaveCount(0);
  });
});
