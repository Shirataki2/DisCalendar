import { expect, type Page, test } from "@playwright/test";
import { createEvent, dayCell, openEventPopover } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

// カレンダーの表示設定 (#96)。最初に表示するビューと週の開始曜日を選べ、テーマ (#58) と同じく
// 選択は localStorage に残る。ダイアログはアカウントメニューとサイドバーの両方から開ける

const guildId = E2E_GUILDS.admin.id;
const createdEventIds: number[] = [];
test.afterEach(async ({ request }) => {
  for (const id of createdEventIds.splice(0)) {
    const response = await request.delete(`/local/api/events/${guildId}/${id}`);
    expect(response.status()).toBe(204);
  }
});

/** アカウントメニューから設定ダイアログを開く */
async function openSettingsFromMenu(page: Page) {
  await page.getByRole("button", { name: "アカウントメニュー" }).click();
  await page
    .getByRole("menu")
    .getByRole("menuitem", { name: "カレンダーの表示設定" })
    .click();
  const dialog = page.getByRole("dialog", { name: "カレンダーの表示設定" });
  await expect(dialog).toBeVisible();
  return dialog;
}

/** ビュー切替タブ (ヘッダツールバー) が選択状態か。FullCalendar v7 は role=tablist / tab で出す */
function viewTab(page: Page, name: string) {
  return page.getByRole("tab", { name, exact: true });
}

test("週の開始を月曜にするとその場で反映され、再読込後も残る", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  // 既定は日曜始まり
  await expect(page.getByRole("columnheader").first()).toHaveText("日");

  const dialog = await openSettingsFromMenu(page);
  await dialog.getByRole("combobox", { name: "週の開始曜日" }).click();
  await page.getByRole("option", { name: "月曜日" }).click();
  // ダイアログはモーダルで、開いている間は背景がアクセシビリティツリーから外れる
  // (getByRole で探せない) ので、閉じてから確認する
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  // 再読込しなくてもその場で反映されている
  await expect(page.getByRole("columnheader").first()).toHaveText("月");

  // localStorage に残るので再読込しても月曜始まりのまま
  await page.reload();
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(page.getByRole("columnheader").first()).toHaveText("月");
});

test("最初に表示するビューを「週」にすると、次に開いたときから週になる", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");

  const dialog = await openSettingsFromMenu(page);
  await dialog.getByRole("combobox", { name: "最初に表示するビュー" }).click();
  await page.getByRole("option", { name: "週", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  // 表示中のビューは変わらない (次にカレンダーを開いたときから使われる)
  await expect(viewTab(page, "月")).toHaveAttribute("aria-selected", "true");

  await page.reload();
  await expect(viewTab(page, "週")).toHaveAttribute("aria-selected", "true");
});

test("「前回開いていたビュー」にするとリストで閉じた後もリストで開く", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();

  const dialog = await openSettingsFromMenu(page);
  await dialog.getByRole("combobox", { name: "最初に表示するビュー" }).click();
  await page.getByRole("option", { name: "前回開いていたビュー" }).click();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  await viewTab(page, "リスト").click();
  await expect(page.getByRole("grid")).toHaveCount(0);

  await page.reload();
  await expect(viewTab(page, "リスト")).toHaveAttribute(
    "aria-selected",
    "true",
  );
  // リストビューなので月のグリッドは出ない
  await expect(page.getByRole("grid")).toHaveCount(0);
});

test("サイドバーからも設定を開ける", async ({ page }) => {
  await page.goto("/dashboard");
  await page
    .getByRole("navigation", { name: "サイト内メニュー" })
    .getByRole("button", { name: "カレンダーの表示設定" })
    .click();
  await expect(
    page.getByRole("dialog", { name: "カレンダーの表示設定" }),
  ).toBeVisible();
});

test.describe("スマートフォンの幅", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true });

  test("ドロワーから設定を開いて変更できる (ドロワーが閉じてもダイアログは残る)", async ({
    page,
  }) => {
    await page.goto(`/dashboard/${guildId}`);
    await expect(page.getByRole("grid")).toBeVisible();

    // ハンバーガーでドロワー (Sheet) を開く。項目を押すとドロワーは閉じるが、
    // ダイアログは DashboardShell 側にあるので開いたまま残る
    // exact を付けないと「アカウントメニュー」にもマッチする (name は部分一致)
    await page.getByRole("button", { name: "メニュー", exact: true }).click();
    await page
      .getByRole("navigation", { name: "サイト内メニュー" })
      .getByRole("button", { name: "カレンダーの表示設定" })
      .click();
    const dialog = page.getByRole("dialog", { name: "カレンダーの表示設定" });
    await expect(dialog).toBeVisible();

    // そのまま設定の変更もできる
    await dialog.getByRole("combobox", { name: "週の開始曜日" }).click();
    await page.getByRole("option", { name: "月曜日" }).click();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(page.getByRole("columnheader").first()).toHaveText("月");
  });
});

async function setCreationDefaults(page: Page) {
  const dialog = await openSettingsFromMenu(page);
  await dialog.getByLabel("既定の色", { exact: true }).click();
  await page.getByRole("button", { name: "#2196F3", exact: true }).click();
  await dialog.getByLabel("クリックで作るときの長さ").click();
  await page.getByRole("option", { name: "1 時間", exact: true }).click();
  await dialog.getByLabel("事前通知の既定値", { exact: true }).click();
  await page.getByRole("option", { name: "自分で指定" }).click();
  await dialog.getByRole("button", { name: "この通知を削除" }).last().click();
  await dialog.getByLabel("通知のタイミング (数値)").fill("30");
  await dialog.getByLabel("通知のタイミング (単位)").click();
  await page.getByRole("option", { name: "分前", exact: true }).click();
  await dialog.getByRole("button", { name: "既定値を保存" }).click();
  await expect(dialog.getByRole("status")).toHaveText("既定値を保存しました。");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
}

test("個人既定値は新規作成と再読込に反映され、編集・複製は元の値を維持する", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  const title = `個人既定値 ${Date.now()}`;
  const created = page.waitForResponse(
    (res) =>
      res.url().includes(`/local/api/events/${guildId}`) &&
      res.request().method() === "POST",
  );
  await createEvent(page, title);
  createdEventIds.push((await (await created).json()).id);
  await setCreationDefaults(page);
  for (const reload of [false, true]) {
    if (reload) await page.reload();
    await page.getByRole("button", { name: "新規作成" }).click();
    const create = page.getByRole("dialog", { name: "予定を作成" });
    await expect(create.getByLabel("色", { exact: true })).toContainText(
      "#2196F3",
    );
    await expect(create.getByLabel("通知のタイミング (数値)")).toHaveValue(
      "30",
    );
    await expect(create.getByLabel("通知のタイミング (単位)")).toContainText(
      "分前",
    );
    const start = await create.getByLabel("開始時刻").inputValue();
    const end = await create.getByLabel("終了時刻").inputValue();
    expect(
      (Number(end.slice(0, 2)) - Number(start.slice(0, 2)) + 24) % 24,
    ).toBe(1);
    await create.getByRole("button", { name: "キャンセル" }).click();
  }
  for (const action of ["編集", "複製"]) {
    const popover = await openEventPopover(page, title);
    await popover.getByRole("button", { name: action }).click();
    const dialog = page.getByRole("dialog", {
      name: action === "編集" ? "予定を編集" : "予定を作成",
    });
    await expect(dialog.getByLabel("色", { exact: true })).toContainText(
      "#F44336",
    );
    await expect(dialog.getByLabel("通知のタイミング (数値)")).toHaveCount(2);
    await dialog.getByRole("button", { name: "キャンセル" }).click();
  }
  const settings = await openSettingsFromMenu(page);
  await settings.getByLabel("事前通知の既定値", { exact: true }).click();
  await page.getByRole("option", { name: "サーバー設定に従う" }).click();
  await settings.getByRole("button", { name: "既定値を保存" }).click();
  await page.keyboard.press("Escape");
  await expect(settings).toBeHidden();
  await page.getByRole("button", { name: "新規作成" }).focus();
  await page.keyboard.press("n");
  const create = page.getByRole("dialog", { name: "予定を作成" });
  await expect(create.getByLabel("通知のタイミング (数値)")).toHaveCount(2);
});

test("不正な通知は保存せず、通知なしも保存できる", async ({ page }) => {
  await page.goto(`/dashboard/${guildId}`);
  await setCreationDefaults(page);
  const dialog = await openSettingsFromMenu(page);
  const number = dialog.getByLabel("通知のタイミング (数値)");
  await number.fill("0");
  // ネイティブの min 検証を含め、保存されないことを再オープンで確かめる。
  await dialog.getByRole("button", { name: "既定値を保存" }).click();
  await page.keyboard.press("Escape");
  const reopened = await openSettingsFromMenu(page);
  await expect(reopened.getByLabel("通知のタイミング (数値)")).toHaveValue(
    "30",
  );
  await reopened.getByRole("button", { name: "この通知を削除" }).click();
  await reopened.getByRole("button", { name: "既定値を保存" }).click();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "新規作成" }).click();
  await expect(
    page.getByRole("dialog").getByLabel("通知のタイミング (数値)"),
  ).toHaveCount(0);
});

for (const hasTouch of [false, true]) {
  test.describe(hasTouch ? "タップで新規作成" : "クリックで新規作成", () => {
    test.use({
      hasTouch,
      viewport: { width: hasTouch ? 375 : 1280, height: 900 },
    });
    test("日付から終日を外すと1時間になり、日付またぎも保存できる", async ({
      page,
    }) => {
      await page.clock.setFixedTime(new Date("2026-09-09T23:00:00+09:00"));
      await page.goto(`/dashboard/${guildId}`);
      await setCreationDefaults(page);
      const day = hasTouch ? 16 : 15;
      const cell = dayCell(page, new Date(2026, 8, day));
      if (hasTouch) await cell.tap({ position: { x: 12, y: 45 } });
      else await cell.click({ position: { x: 12, y: 45 } });
      const dialog = page.getByRole("dialog", { name: "予定を作成" });
      await expect(
        dialog.getByRole("checkbox", { name: "終日", exact: true }),
      ).toBeChecked();
      await dialog
        .getByRole("checkbox", { name: "終日", exact: true })
        .uncheck();
      await expect(dialog.getByLabel("開始時刻")).toHaveValue("23:00");
      await expect(dialog.getByLabel("終了時刻")).toHaveValue("00:00");
      await dialog.getByLabel("タイトル").fill(`深夜の既定値 ${hasTouch}`);
      const response = page.waitForResponse(
        (res) =>
          res.url().includes(`/local/api/events/${guildId}`) &&
          res.request().method() === "POST",
      );
      await dialog.getByRole("button", { name: "作成", exact: true }).click();
      const result = await response;
      expect(result.status()).toBe(201);
      createdEventIds.push((await result.json()).id);
      expect(result.request().postDataJSON()).toMatchObject({
        start_at: `2026-09-${day}T23:00:00`,
        end_at: `2026-09-${day + 1}T00:00:00`,
      });
    });
  });
}

test("時間枠のクリックは1時間、ドラッグは選んだ範囲を使う", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await setCreationDefaults(page);
  await page.getByRole("tab", { name: "日", exact: true }).click();
  const slot = page.locator('[data-time="10:00:00"]').last();
  await slot.scrollIntoViewIfNeeded();
  const box = await slot.boundingBox();
  if (!box) throw new Error("時間枠が表示されていません");
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 4;
  await page.mouse.click(x, y);
  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await expect(dialog.getByLabel("開始時刻")).toHaveValue("10:00");
  await expect(dialog.getByLabel("終了時刻")).toHaveValue("11:00");
  await dialog.getByRole("button", { name: "キャンセル" }).click();
  await expect(dialog).toBeHidden();
  const target = await page
    .locator('[data-time="11:30:00"]')
    .last()
    .boundingBox();
  if (!target) throw new Error("選択先の時間枠が表示されていません");
  await page.mouse.move(x, y);
  await page.mouse.down();
  // 15分刻みなので、11:30の枠まで選ぶと終了は11:45になる。
  await page.mouse.move(x, target.y + target.height / 4, { steps: 15 });
  await page.mouse.up();
  await expect(dialog.getByLabel("開始時刻")).toHaveValue("10:00");
  await expect(dialog.getByLabel("終了時刻")).toHaveValue("11:45");
});

test.describe("通知10件の表示", () => {
  test.use({ viewport: { width: 375, height: 812 }, hasTouch: true });
  test("スマートフォンで通知10件を編集・保存できる", async ({
    page,
  }, testInfo) => {
    await page.goto(`/dashboard/${guildId}`);
    await setCreationDefaults(page);
    const dialog = await openSettingsFromMenu(page);
    const add = dialog.getByRole("button", { name: "通知を追加" });
    for (let i = 1; i < 10; i++) await add.click();
    await expect(add).toBeDisabled();
    await expect(dialog.getByLabel("通知のタイミング (数値)")).toHaveCount(10);
    await dialog.getByRole("button", { name: "既定値を保存" }).click();
    await expect(dialog.getByRole("status")).toBeVisible();
    await dialog.screenshot({
      path: testInfo.outputPath("creation-defaults-mobile.png"),
    });
    await page.keyboard.press("Escape");
    const reopened = await openSettingsFromMenu(page);
    await expect(reopened.getByLabel("通知のタイミング (数値)")).toHaveCount(
      10,
    );
    await reopened.screenshot({
      path: testInfo.outputPath("creation-defaults-mobile-top.png"),
    });
  });
});
