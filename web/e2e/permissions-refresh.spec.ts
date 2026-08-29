import { expect, type Page, test } from "@playwright/test";
import { addDays, calendarToday, dayCell } from "./calendar";
import { setGuildEventPermissions } from "./discord-mock";
import { E2E_GUILDS } from "./fixtures";

// 権限を直した直後に「権限を再確認」で反映させる (#122)。
// api は Discord の権限を数分キャッシュするので、案内どおりに Bot を招待し直しても
// そのままでは待たされる。ボタンでキャッシュを捨てて取り直せることを確かめる。
//
// 権限を付け替えるギルドは、api 側のキャッシュが変わったまま残るので専用のものを使う
// (他のテストと共有すると、そちらの前提が崩れる)

test.describe.configure({ mode: "serial" });

/** 明日のセルから作成ダイアログを開く (終日予定になり、開始が未来なので連携できる) */
async function openCreateDialog(page: Page) {
  const tomorrow = addDays(await calendarToday(page), 1);
  const box = await dayCell(page, tomorrow).boundingBox();
  if (!box) throw new Error("日付セルが表示されていません");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height - 10);
  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await expect(dialog).toBeVisible();
  return dialog;
}

test("Bot を招待し直したら「権限を再確認」でその場で使えるようになる", async ({
  page,
}) => {
  const guild = E2E_GUILDS.reinvited;
  await page.goto(`/dashboard/${guild.id}`);
  await expect(page.getByRole("grid")).toBeVisible();
  const dialog = await openCreateDialog(page);
  const checkbox = dialog.getByRole("checkbox", {
    name: "Discord のイベントとしても作成する",
  });
  await expect(checkbox).toBeDisabled();

  // Bot を招待し直して「イベントの作成」権限が付いた状態にする。
  // api のキャッシュはまだ古いので、この時点では表示は変わらない
  await setGuildEventPermissions(guild.id, {
    botCreateEvents: true,
    userCreateEvents: true,
  });
  await expect(checkbox).toBeDisabled();

  await dialog.getByRole("button", { name: "権限を再確認" }).click();
  await expect(checkbox).toBeEnabled();
  // 使えるようになったらボタンごと消える
  await expect(
    dialog.getByRole("button", { name: "権限を再確認" }),
  ).toBeHidden();
  await dialog.getByRole("button", { name: "キャンセル" }).click();
  await expect(dialog).toBeHidden();
});

test("権限が変わっていなければ、再確認しても使えないままで案内が出る", async ({
  page,
}) => {
  // 自分の権限が足りないギルド (Bot の招待し直しでは直らない方)。
  // 何も変えずに押すので、api のキャッシュを捨てても結果は同じ
  const guild = E2E_GUILDS.noUserEventsPerm;
  await page.goto(`/dashboard/${guild.id}`);
  await expect(page.getByRole("grid")).toBeVisible();
  const dialog = await openCreateDialog(page);
  const checkbox = dialog.getByRole("checkbox", {
    name: "Discord のイベントとしても作成する",
  });
  await expect(checkbox).toBeDisabled();

  await dialog.getByRole("button", { name: "権限を再確認" }).click();
  await expect(
    dialog.getByText("Discord 側の権限はまだ変わっていません"),
  ).toBeVisible();
  await expect(checkbox).toBeDisabled();
  await dialog.getByRole("button", { name: "キャンセル" }).click();
  await expect(dialog).toBeHidden();
});

test.afterAll(async () => {
  // モック側は元に戻す (api のキャッシュは戻せないので、このギルドは他のテストで使わない)
  await setGuildEventPermissions(E2E_GUILDS.reinvited.id, null);
});
