import { expect, type Page, test } from "@playwright/test";
import {
  addDays,
  calendarToday,
  dayCell,
  eventOn,
  openEventTab,
} from "./calendar";
import { scheduledEventImage, setImageFailure } from "./discord-mock";
import { E2E_GUILDS } from "./fixtures";

// 予定を Discord のスケジュールイベントとしても作成する (#94)。
// Discord 側はモック (discord-mock.ts)。連携には Bot とユーザー本人の両方に
// 「イベントの作成」権限が要るので、ギルドごとに組み合わせを変えてある:
// - admin: 両方あり (ユーザーはオーナー = ADMINISTRATOR)
// - noEventsPerm: ユーザーにはあるが Bot にない
// - noUserEventsPerm: Bot にはあるがユーザーにない

const guildId = E2E_GUILDS.admin.id;
const eventsApi = new RegExp(`/local/api/events/${guildId}(/|\\?|$)`);

test.describe.configure({ mode: "serial" });

const stamp = Date.now().toString(36);
const linkedTitle = `E2E Discord 連携 ${stamp}`;

/**
 * 日付セルの下端付近をクリックして詳細作成ダイアログを開く
 * (セルの中央だと、他のテストが同じ日に置いた予定に当たることがある)
 */
async function clickDayCell(page: Page, date: Date) {
  const box = await dayCell(page, date).boundingBox();
  if (!box) throw new Error("日付セルが表示されていません");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height - 10);
  await page
    .getByRole("dialog", { name: "予定をクイック追加" })
    .getByRole("button", { name: "詳細を入力" })
    .click();
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

// 複製は「チェック済みの新規作成」なので、連携済みの編集と違って権限の免除は効かない
// (権限のあるこのギルドでは、チェックが引き継がれて操作できる)
test("連携済みの予定を複製すると、チェックを引き継いだ複製ダイアログになる", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  await eventOn(page, linkedTitle).click();
  const popover = page.getByRole("dialog").filter({ hasText: linkedTitle });
  await popover.getByRole("button", { name: "複製" }).click();
  const dialog = page.getByRole("dialog", { name: "予定を複製" });
  await expect(dialog).toBeVisible();
  const checkbox = dialog.getByRole("checkbox", {
    name: "Discord のイベントとしても作成する",
  });
  await expect(checkbox).toBeChecked();
  await expect(checkbox).toBeEnabled();
  await dialog.getByRole("button", { name: "キャンセル" }).click();
  await expect(dialog).toBeHidden();
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

test("自分に「イベントの作成」権限がないサーバーではチェックできず、案内が出る", async ({
  page,
}) => {
  await page.goto(`/dashboard/${E2E_GUILDS.noUserEventsPerm.id}`);
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
  // Bot 権限の不足とは案内が違う (再招待では直らないので、ロールの確認を促す)
  await expect(
    dialog.getByText(
      "Discord の「イベントの作成」権限を持つ人だけが利用できます",
    ),
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

test("Bot に権限のないサーバーへの連携作成は API 側でも拒否される (403)", async ({
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

// 連携は Bot が代行するので、本人の権限を見ないと web 経由で権限昇格できてしまう (#94)
test("自分に権限のないサーバーへの連携作成は API 側でも拒否される (403)", async ({
  page,
}) => {
  const guild = E2E_GUILDS.noUserEventsPerm.id;
  const res = await page.request.post(`/local/api/events/${guild}`, {
    data: {
      name: "本人権限なし",
      notifications: [],
      color: "#2196F3",
      is_all_day: false,
      start_at: "2030-01-01T10:00:00",
      end_at: "2030-01-01T11:00:00",
      discord_scheduled_event: true,
    },
  });
  expect(res.status()).toBe(403);
  const list = await page.request.get(
    `/local/api/events/${guild}?start=2030-01-01T00:00:00&end=2030-01-02T00:00:00`,
  );
  expect(
    ((await list.json()) as { name: string }[]).some(
      (e) => e.name === "本人権限なし",
    ),
  ).toBe(false);

  // 連携なしの予定は今までどおり作れる (制限するのは連携だけ)
  const plain = await page.request.post(`/local/api/events/${guild}`, {
    data: {
      name: "本人権限なし (連携なし)",
      notifications: [],
      color: "#2196F3",
      is_all_day: false,
      start_at: "2030-01-01T10:00:00",
      end_at: "2030-01-01T11:00:00",
      discord_scheduled_event: false,
    },
  });
  expect(plain.status()).toBe(201);
  const created = (await plain.json()) as { id: number };
  expect(
    (
      await page.request.delete(`/local/api/events/${guild}/${created.id}`)
    ).status(),
  ).toBe(204);
});

// 添付画像を Discord イベントのカバーにする (#403)
test("添付した画像を Discord イベントのカバーにでき、解除・反映の失敗と再試行ができる", async ({
  page,
}) => {
  const png = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a3l8AAAAASUVORK5CYII=",
    "base64",
  );
  const title = `E2E カバー ${stamp}`;
  await page.goto(`/dashboard/${guildId}`);
  await expect(page.getByRole("grid")).toBeVisible();
  const day = addDays(await calendarToday(page), 2);
  await clickDayCell(page, day);
  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await dialog.getByLabel("タイトル").fill(title);

  // 連携がオフのうちはカバーの選択肢を出さない
  await openEventTab(dialog, "添付・共有");
  await dialog.getByLabel("ファイルを添付", { exact: true }).setInputFiles([
    { name: "告知.png", mimeType: "image/png", buffer: png },
    {
      name: "資料.pdf",
      mimeType: "application/pdf",
      buffer: Buffer.from("%PDF-1.7\n%%EOF"),
    },
  ]);
  await expect(
    dialog.getByRole("button", { name: "Discord のカバーにする" }),
  ).toHaveCount(0);

  await openEventTab(dialog, "基本");
  await dialog
    .getByRole("checkbox", { name: "Discord のイベントとしても作成する" })
    .check();
  await openEventTab(dialog, "添付・共有");
  // 画像だけに選択肢が出る (PDF には出ない)
  await expect(
    dialog.getByRole("button", { name: "Discord のカバーにする" }),
  ).toHaveCount(1);
  await dialog.getByRole("button", { name: "Discord のカバーにする" }).click();
  await expect(
    dialog.getByText("Discord のカバー", { exact: true }),
  ).toBeVisible();

  const created = page.waitForResponse(
    (res) => eventsApi.test(res.url()) && res.request().method() === "POST",
  );
  const covered = page.waitForResponse(
    (res) =>
      res.url().endsWith("/attachments/cover") &&
      res.request().method() === "PUT",
  );
  await dialog.getByRole("button", { name: "作成" }).click();
  const event = (await (await created).json()) as {
    id: number;
    discord_scheduled_event_id: string;
  };
  expect((await (await covered).json()).discord.status).toBe("synced");
  await expect(dialog).toBeHidden();
  expect(await scheduledEventImage(event.discord_scheduled_event_id)).toMatch(
    /^data:image\/png;base64,/,
  );

  try {
    // 編集ダイアログからカバーを外すと Discord 側の画像も外れる
    await eventOn(dayCell(page, day), title).click();
    await page
      .getByRole("dialog")
      .filter({ hasText: title })
      .getByRole("button", { name: "編集" })
      .click();
    const edit = page.getByRole("dialog", { name: "予定を編集" });
    await openEventTab(edit, "添付・共有");
    await edit.getByRole("button", { name: "カバーを外す" }).click();
    await expect(
      edit.getByRole("button", { name: "Discord のカバーにする" }),
    ).toBeVisible();
    await expect
      .poll(() => scheduledEventImage(event.discord_scheduled_event_id))
      .toBeNull();

    // Discord への反映に失敗してもカバーの指定は保存され、失敗が分かり、再試行できる
    await setImageFailure(true);
    await edit.getByRole("button", { name: "Discord のカバーにする" }).click();
    await expect(
      edit.getByText("Discord のイベントに反映できませんでした", {
        exact: false,
      }),
    ).toBeVisible();
    const saved = (await (
      await page.request.get(
        `/local/api/events/${guildId}/${event.id}/attachments`,
      )
    ).json()) as { filename: string; discord_cover: boolean }[];
    expect(saved.find((a) => a.filename === "告知.png")?.discord_cover).toBe(
      true,
    );
    expect(
      await scheduledEventImage(event.discord_scheduled_event_id),
    ).toBeNull();
    await setImageFailure(false);
    await edit
      .getByRole("button", { name: "Discord への反映を再試行" })
      .click();
    await expect(
      edit.getByText("Discord のイベントに反映できませんでした", {
        exact: false,
      }),
    ).toBeHidden();
    expect(await scheduledEventImage(event.discord_scheduled_event_id)).toMatch(
      /^data:image\/png;base64,/,
    );

    // カバーの添付を消すと Discord 側の画像も外れる
    page.once("dialog", (confirm) => confirm.accept());
    await edit.getByRole("button", { name: "添付を削除" }).first().click();
    await expect
      .poll(() => scheduledEventImage(event.discord_scheduled_event_id))
      .toBeNull();
    await edit.getByRole("button", { name: "キャンセル" }).click();
  } finally {
    await setImageFailure(false);
    await page.request.delete(`/local/api/events/${guildId}/${event.id}`);
  }
});
