import { expect, type Page, test } from "@playwright/test";
import { addDays, calendarToday, dayCell, eventOn } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

// 予定を Discord のスケジュールイベントとしても作成する (#94)。
// Discord 側はモック (discord-mock.ts) で、admin ギルドは「イベントの作成」あり、
// noEventsPerm ギルドはなしに設定してある

const guildId = E2E_GUILDS.admin.id;
const eventsApi = new RegExp(`/local/api/events/${guildId}(/|\\?|$)`);

test.describe.configure({ mode: "serial" });

const stamp = Date.now().toString(36);
const linkedTitle = `E2E Discord 連携 ${stamp}`;

/**
 * 日付セルの下端付近をクリックして作成ダイアログを開く
 * (セルの中央だと、他のテストが同じ日に置いた予定に当たることがある)
 */
async function clickDayCell(page: Page, date: Date) {
  const box = await dayCell(page, date).boundingBox();
  if (!box) throw new Error("日付セルが表示されていません");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height - 10);
}

test("チェックを入れて作成すると連携され、編集ダイアログを開き直してもチェックが残る", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();

  // 既定の「新規作成」の日時 (今の HH:00 開始) は過去になるので、明日のセルから作る
  // (終日予定になり、開始が明日 0:00 = 未来なのでチェックできる)
  const tomorrow = addDays(await calendarToday(page), 1);
  await clickDayCell(page, tomorrow);
  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("タイトル").fill(linkedTitle);
  const checkbox = dialog.getByRole("checkbox", {
    name: "Discord のイベントとしても作成する",
  });
  await expect(checkbox).toBeEnabled();
  await checkbox.check();

  const created = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "POST",
  );
  await dialog.getByRole("button", { name: "作成" }).click();
  const createdRes = await created;
  expect(createdRes.status()).toBe(201);
  // API が連携先のイベント ID を返す (モックが採番したもの)
  expect(
    ((await createdRes.json()) as { discord_scheduled_event_id: string | null })
      .discord_scheduled_event_id,
  ).not.toBeNull();
  await expect(dialog).toBeHidden();

  // 編集ダイアログを開き直すと、DB の対応付けからチェックが入った状態で読み込まれる
  await eventOn(dayCell(page, tomorrow), linkedTitle).click();
  const popover = page.getByRole("dialog").filter({ hasText: linkedTitle });
  await popover.getByRole("button", { name: "編集" }).click();
  const editDialog = page.getByRole("dialog", { name: "予定を編集" });
  await expect(
    editDialog.getByRole("checkbox", {
      name: "Discord のイベントとしても作成する",
    }),
  ).toBeChecked();
  await editDialog.getByRole("button", { name: "キャンセル" }).click();
  await expect(editDialog).toBeHidden();
});

test("後続のテストに予定を残さない (削除で Discord 側も消える)", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  await eventOn(page, linkedTitle).click();
  const popover = page.getByRole("dialog").filter({ hasText: linkedTitle });
  await popover.getByRole("button", { name: "削除" }).click();
  const deleted = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "DELETE",
  );
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "削除" })
    .click();
  expect((await deleted).status()).toBe(204);
  await expect(eventOn(page, linkedTitle)).toHaveCount(0);
});

test("Bot に「イベントの作成」権限がないサーバーではチェックできず、案内が出る", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.noEventsPerm.id}`);
  await expect(page.getByRole("grid")).toBeVisible();
  const tomorrow = addDays(await calendarToday(page), 1);
  await clickDayCell(page, tomorrow);
  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole("checkbox", {
      name: "Discord のイベントとしても作成する",
    }),
  ).toBeDisabled();
  await expect(
    dialog.getByText("Bot に「イベントの作成」権限がないため利用できません"),
  ).toBeVisible();
  await dialog.getByRole("button", { name: "キャンセル" }).click();
  await expect(dialog).toBeHidden();
});

// 表示だけの制御ではないことの確認 (settings.spec.ts と同じ流儀で API を直接叩く)

test("開始が過去の予定は API 側でも連携を拒否する (400)", async ({ page }) => {
  const res = await page.request.post(`/local/api/events/${guildId}`, {
    data: {
      name: "過去開始",
      notifications: [],
      color: "#2196F3",
      is_all_day: false,
      start_at: "2020-01-01T10:00:00",
      end_at: "2020-01-01T11:00:00",
      discord_scheduled_event: true,
    },
  });
  expect(res.status()).toBe(400);
});

test("権限のないサーバーへの連携作成は API 側でも拒否される (403)", async ({
  page,
}) => {
  const res = await page.request.post(
    `/local/api/events/${E2E_GUILDS.noEventsPerm.id}`,
    {
      data: {
        name: "権限なし",
        notifications: [],
        color: "#2196F3",
        is_all_day: false,
        start_at: "2030-01-01T10:00:00",
        end_at: "2030-01-01T11:00:00",
        discord_scheduled_event: true,
      },
    },
  );
  expect(res.status()).toBe(403);
  // Discord 側が失敗したら予定も作られない (全体を失敗にする)
  const list = await page.request.get(
    `/local/api/events/${E2E_GUILDS.noEventsPerm.id}?start=2030-01-01T00:00:00&end=2030-01-02T00:00:00`,
  );
  expect(
    ((await list.json()) as { name: string }[]).some(
      (e) => e.name === "権限なし",
    ),
  ).toBe(false);
});
