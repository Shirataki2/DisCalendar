import { expect, test } from "@playwright/test";
import {
  calendarToday,
  createEvent,
  dayCell,
  dragEventTo,
  eventOn,
  neighborDay,
  openEventPopover,
} from "./calendar";
import { E2E_GUILDS, E2E_USER } from "./fixtures";

// 予定の作成 → 編集 → ドラッグ移動 (成功 / 失敗時のロールバック) → 削除。
// 管理権限のあるギルド (E2E_GUILDS.admin) で、同じブラウザで順に進める

const guildId = E2E_GUILDS.admin.id;
const eventsApi = new RegExp(`/local/api/events/${guildId}(/|\\?|$)`);

test.describe.configure({ mode: "serial" });

// テスト間で同じ名前の予定が残らないよう、実行ごとに変える。
// serial グループは失敗すると新しいワーカーで先頭から再実行される (retries) ので、そのときも別の名前になり、
// 前の試行で残った予定とは区別できる
const stamp = Date.now().toString(36);
const createdTitle = `E2E 予定 ${stamp}`;
const editedTitle = `E2E 予定 ${stamp} (編集済み)`;
const duplicatedTitle = `E2E 予定 ${stamp} (複製)`;

test.beforeEach(async ({ page }) => {
  // 23時台も既定の「次の正時」が翌日にならないよう、日本時間の正午で確認する。
  const noon = new Date();
  noon.setUTCHours(3, 0, 0, 0);
  await page.clock.setFixedTime(noon);
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
});

test("予定を作成するとカレンダーに出て、再読込しても残る", async ({ page }) => {
  const created = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "POST",
  );
  await createEvent(page, createdTitle);
  expect((await created).status()).toBe(201);

  // 既定の日時は今日 (ブラウザの時刻) なので、今日のセルに入っている
  const today = await calendarToday(page);
  await expect(eventOn(dayCell(page, today), createdTitle)).toBeVisible();
  await page.reload();
  await expect(eventOn(page, createdTitle)).toBeVisible();
});

test("予定をクリックすると概要が出て、編集ダイアログから保存できる", async ({
  page,
}) => {
  const popover = await openEventPopover(page, createdTitle);
  await expect(popover).toContainText(`作成:${E2E_USER.name}`);
  await expect(popover).not.toContainText("最終更新:");
  // 旧フォームの既定の通知
  await expect(popover).toContainText("1日前・1時間前");
  await popover.getByRole("button", { name: "編集" }).click();

  const dialog = page.getByRole("dialog", { name: "予定を編集" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("タイトル")).toHaveValue(createdTitle);
  await dialog.getByLabel("タイトル").fill(editedTitle);
  await dialog.getByLabel("説明").fill("E2E で編集した説明");
  await dialog
    .getByLabel("場所 / URL")
    .fill("https://meet.example.com/e2e-room");
  const updated = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "PUT",
  );
  await dialog.getByRole("button", { name: "保存" }).click();
  expect((await updated).status()).toBe(200);
  await expect(dialog).toBeHidden();

  await expect(eventOn(page, editedTitle)).toBeVisible();
  await expect(eventOn(page, createdTitle)).toHaveCount(0);
  const reopened = await openEventPopover(page, editedTitle);
  await expect(reopened).toContainText("E2E で編集した説明");
  await expect(
    reopened.getByRole("link", { name: "https://meet.example.com/e2e-room" }),
  ).toHaveAttribute("rel", "noopener noreferrer");
  await expect(reopened).toContainText(`最終更新:${E2E_USER.name}`);
});

test("複製すると元の内容が入った複製ダイアログが開き、新しい予定として保存できる", async ({
  page,
}) => {
  const popover = await openEventPopover(page, editedTitle);
  await popover.getByRole("button", { name: "複製" }).click();

  // 元のタイトル・説明が入った「複製」ダイアログが開く
  const dialog = page.getByRole("dialog", { name: "予定を複製" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel("タイトル")).toHaveValue(editedTitle);
  await expect(dialog.getByLabel("説明")).toHaveValue("E2E で編集した説明");
  await expect(dialog.getByLabel("場所 / URL")).toHaveValue(
    "https://meet.example.com/e2e-room",
  );
  const inherited = dialog.getByText("元の予定の日時を引き継いでいます。", {
    exact: false,
  });
  await expect(inherited).toBeVisible();
  const today = await calendarToday(page);
  await expect(inherited).toContainText(
    `${today.getFullYear()}/${today.getMonth() + 1}/${today.getDate()}`,
  );
  await expect(inherited).toContainText(
    await dialog.getByLabel("開始時刻").inputValue(),
  );
  await expect(inherited).toContainText(
    await dialog.getByLabel("終了時刻").inputValue(),
  );
  await dialog.getByLabel("タイトル").fill(duplicatedTitle);
  const created = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "POST",
  );
  await dialog.getByRole("button", { name: "作成" }).click();
  expect((await created).status()).toBe(201);
  await expect(dialog).toBeHidden();

  // 元の予定は残ったまま複製が増え、通知設定も引き継がれている
  await expect(eventOn(page, editedTitle)).toBeVisible();
  const reopened = await openEventPopover(page, duplicatedTitle);
  await expect(reopened).toContainText("1日前・1時間前");

  // 後続のテスト (ドラッグ・削除) の対象を editedTitle だけに保つため、複製は消しておく
  await reopened.getByRole("button", { name: "削除" }).click();
  const deleted = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "DELETE",
  );
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "削除" })
    .click();
  expect((await deleted).status()).toBe(204);
  await expect(eventOn(page, duplicatedTitle)).toHaveCount(0);
});

test("タイトルが空のままでは作成できない (フォームの検証)", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "新規作成" }).click();
  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  const location = dialog.getByLabel("場所 / URL");
  await expect(location).toBeVisible();
  await location.fill("あ".repeat(201));
  await dialog.getByRole("button", { name: "作成" }).click();
  await expect(
    dialog.getByText("場所 / URL は200文字以内で入力してください"),
  ).toBeVisible();
  await location.fill("");
  await location.focus();
  await page.keyboard.press("Tab");
  await expect(dialog.getByLabel("説明")).toBeFocused();
  await dialog.getByRole("button", { name: "作成" }).click();
  await expect(dialog.getByText("タイトルを入力してください")).toBeVisible();
  await dialog.getByRole("button", { name: "キャンセル" }).click();
  await expect(dialog).toBeHidden();
});

test("ドラッグで別の日に移動すると API に保存される", async ({ page }) => {
  const today = await calendarToday(page);
  const target = await neighborDay(page, today);

  const updated = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "PUT",
  );
  await dragEventTo(
    page,
    eventOn(dayCell(page, today), editedTitle),
    dayCell(page, target),
  );
  expect((await updated).status()).toBe(200);
  await expect(eventOn(dayCell(page, target), editedTitle)).toBeVisible();
  await expect(eventOn(dayCell(page, today), editedTitle)).toHaveCount(0);

  // 再読込しても移動後の日付にある (サーバーに保存された)
  await page.reload();
  await expect(eventOn(dayCell(page, target), editedTitle)).toBeVisible();
});

test("変更履歴に作成・編集・ドラッグでの変更が差分で出る (#165)", async ({
  page,
}) => {
  const popover = await openEventPopover(page, editedTitle);
  const loaded = page.waitForResponse(
    (res) =>
      res.url().includes(`/local/api/events/${guildId}/`) &&
      res.url().endsWith("/history"),
  );
  await popover.getByRole("button", { name: "変更履歴" }).click();
  expect((await loaded).status()).toBe(200);
  const dialog = page.getByRole("dialog", { name: "変更履歴" });
  await expect(dialog).toBeVisible();

  const entries = dialog.getByTestId("event-history").locator(":scope > li");
  // 新しい順: ドラッグ → 編集 → 作成
  await expect(entries).toHaveCount(3);
  await expect(entries.nth(0)).toContainText(E2E_USER.name);
  await expect(entries.nth(0)).toContainText("日時:");
  await expect(entries.nth(0)).not.toContainText("タイトル:");
  await expect(entries.nth(1)).toContainText(
    `タイトル: 「${createdTitle}」→「${editedTitle}」`,
  );
  await expect(entries.nth(1)).toContainText(
    "場所: なし → https://meet.example.com/e2e-room",
  );
  await expect(entries.nth(1)).toContainText("説明を変更");
  await expect(entries.nth(1)).not.toContainText("日時:");
  await expect(entries.nth(2)).toContainText("予定を作成しました");

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

test("保存に失敗したドラッグは元の位置に戻り、エラーが表示される", async ({
  page,
}) => {
  const today = await calendarToday(page);
  const from = await neighborDay(page, today);
  await expect(eventOn(dayCell(page, from), editedTitle)).toBeVisible();

  // 更新 API だけ 500 を返させる (ブラウザ側のロールバックを確認する)
  await page.route(eventsApi, async (route) => {
    if (route.request().method() !== "PUT") return route.continue();
    await route.fulfill({
      status: 500,
      contentType: "application/json",
      body: JSON.stringify({ error: "internal_error", message: "e2e" }),
    });
  });
  await dragEventTo(
    page,
    eventOn(dayCell(page, from), editedTitle),
    dayCell(page, today),
  );
  await expect(
    page.getByText("サーバーでエラーが発生しました (500)"),
  ).toBeVisible();
  await expect(eventOn(dayCell(page, from), editedTitle)).toBeVisible();
  await expect(eventOn(dayCell(page, today), editedTitle)).toHaveCount(0);
  await page.unroute(eventsApi);

  const language = page.getByRole("combobox", { name: "Language / 言語" });
  await language.selectOption("en");
  await expect(
    page.getByRole("alert").filter({ hasText: "error occurred" }),
  ).toContainText("A server error occurred (500)");
  await language.selectOption("ja");
  await expect(
    page.getByRole("alert").filter({ hasText: "サーバーでエラー" }),
  ).toContainText("サーバーでエラーが発生しました (500)");

  // エラー表示は閉じられる。再読込しても元の日付のまま
  await page.getByRole("button", { name: "閉じる" }).click();
  await expect(
    page.getByText("サーバーでエラーが発生しました (500)"),
  ).toHaveCount(0);
  await page.reload();
  await expect(eventOn(dayCell(page, from), editedTitle)).toBeVisible();
});

test("削除を確認するとカレンダーから消える", async ({ page }) => {
  const popover = await openEventPopover(page, editedTitle);
  await popover.getByRole("button", { name: "削除" }).click();

  const confirm = page.getByRole("alertdialog");
  await expect(confirm).toContainText("予定を削除しますか？");
  await expect(confirm).toContainText(editedTitle);
  const deleted = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "DELETE",
  );
  await confirm.getByRole("button", { name: "削除" }).click();
  expect((await deleted).status()).toBe(204);

  await expect(eventOn(page, editedTitle)).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(eventOn(page, editedTitle)).toHaveCount(0);
});

// 上のテストで消した予定はゴミ箱 (#159) に残っている。ここでは作り直した予定で「元に戻す」を確かめる
test("削除した直後に「元に戻す」を押すと、同じ日に予定が戻る", async ({
  page,
}) => {
  const title = `E2E 元に戻す ${stamp}`;
  await createEvent(page, title);
  const today = await calendarToday(page);
  await expect(eventOn(dayCell(page, today), title)).toBeVisible();

  const popover = await openEventPopover(page, title);
  await popover.getByRole("button", { name: "削除" }).click();
  const confirm = page.getByRole("alertdialog");
  // 取り消せない削除ではなくなったので、戻し方を案内する
  await expect(confirm).toContainText("30 日間");
  await expect(confirm).not.toContainText("取り消せません");
  await confirm.getByRole("button", { name: "削除" }).click();
  await expect(eventOn(page, title)).toHaveCount(0);

  const notice = page.getByRole("status").filter({ hasText: title });
  await expect(notice).toContainText(`「${title}」を削除しました`);
  const restored = page.waitForResponse(
    (res) =>
      res.url().endsWith("/restore") && res.request().method() === "POST",
  );
  await notice.getByRole("button", { name: "元に戻す" }).click();
  const restoredResponse = await restored;
  expect(restoredResponse.status()).toBe(200);
  const { id } = await restoredResponse.json();
  await expect(notice).toHaveCount(0);

  // 元の日に戻り、再読込しても残る
  await expect(eventOn(dayCell(page, today), title)).toBeVisible();
  await page.reload();
  await expect(eventOn(dayCell(page, today), title)).toBeVisible();
  // 変更履歴に削除と復元が残る
  const reopened = await openEventPopover(page, title);
  await reopened.getByRole("button", { name: "変更履歴" }).click();
  const history = page.getByRole("dialog", { name: "変更履歴" });
  await expect(history).toContainText("予定を削除しました");
  await expect(history).toContainText("予定を復元しました");

  // 他のテストに予定を残さない
  const cleanup = await page.request.delete(
    `/local/api/events/${guildId}/${id}`,
  );
  expect(cleanup.status()).toBe(204);
});
