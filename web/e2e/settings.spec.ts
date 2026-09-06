import { expect, type Page, test } from "@playwright/test";
import { E2E_CHANNELS, E2E_GUILDS } from "./fixtures";

// サーバー設定 (restricted モード・通知の設定 #181) の切替と、管理権限のないユーザーの表示。
// admin ギルドではテストユーザーがオーナー、member ギルドでは権限のない一般メンバー (Discord モックの定義)

const RESTRICTED_NOTICE =
  "このサーバーでは管理権限を持つユーザーのみ予定を編集できます";
const RESTRICTED_LABEL = "予定の追加・編集・削除を";
const NOTIFY_AT_START_LABEL = "予定の開始時刻に通知する";
const CHANNEL_LABEL = "通知先チャンネル";
const configApi = `/local/api/guilds/${E2E_GUILDS.admin.id}/config`;

async function openSettings(page: Page) {
  await page.getByRole("button", { name: "サーバー設定" }).click();
  const dialog = page.getByRole("dialog", { name: "サーバー設定" });
  await expect(dialog).toBeVisible();
  return dialog;
}

/**
 * 各テストの開始時に API で restricted を決め打ちにする (ブラウザと同じ cookie で叩く)。
 * 前のテストの結果や retry で再実行されたときの状態に依存しないようにするため
 */
async function setRestricted(page: Page, restricted: boolean) {
  const res = await page.request.put(configApi, { data: { restricted } });
  expect(res.status()).toBe(200);
}

/** 通知の設定 (#181) を既定 (開始時刻に通知する、1 日前と 1 時間前) に戻す。restricted は変えない */
async function resetNotificationSettings(page: Page) {
  const res = await page.request.put(configApi, {
    data: {
      restricted: false,
      notify_at_start: true,
      default_notifications: [
        { num: 1, unit: "days" },
        { num: 1, unit: "hours" },
      ],
    },
  });
  expect(res.status()).toBe(200);
}

/** 「保存」を押して PUT の完了とダイアログが閉じるのを待つ */
async function saveSettings(page: Page, dialog: ReturnType<Page["getByRole"]>) {
  const saved = page.waitForResponse(
    (res) => res.url().includes(configApi) && res.request().method() === "PUT",
  );
  await dialog.getByRole("button", { name: "保存" }).click();
  expect((await saved).status()).toBe(200);
  await expect(dialog).toBeHidden();
}

test.describe("管理権限のあるギルド", () => {
  test("restricted を有効にして保存すると、再読込後も有効で自分は編集できる", async ({
    page,
  }) => {
    await setRestricted(page, false);
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    await expect(page.getByText(RESTRICTED_NOTICE)).toHaveCount(0);

    const dialog = await openSettings(page);
    const checkbox = dialog.getByRole("checkbox", { name: RESTRICTED_LABEL });
    await expect(checkbox).toBeEnabled();
    await expect(checkbox).not.toBeChecked();
    await checkbox.check();
    await saveSettings(page, dialog);

    await page.reload();
    const reopened = await openSettings(page);
    await expect(
      reopened.getByRole("checkbox", { name: RESTRICTED_LABEL }),
    ).toBeChecked();
    await reopened.getByRole("button", { name: "キャンセル" }).click();
    // 管理権限があるので restricted でも編集できる (注意書きも出ない)
    await expect(page.getByRole("button", { name: "新規作成" })).toBeEnabled();
    await expect(page.getByText(RESTRICTED_NOTICE)).toHaveCount(0);
  });

  test("restricted を無効に戻せる", async ({ page }) => {
    await setRestricted(page, true);
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    const dialog = await openSettings(page);
    const checkbox = dialog.getByRole("checkbox", { name: RESTRICTED_LABEL });
    await expect(checkbox).toBeChecked();
    await checkbox.uncheck();
    await dialog.getByRole("button", { name: "保存" }).click();
    await expect(dialog).toBeHidden();

    await page.reload();
    const reopened = await openSettings(page);
    await expect(
      reopened.getByRole("checkbox", { name: RESTRICTED_LABEL }),
    ).not.toBeChecked();
  });
});

test.describe("通知の設定 (#181)", () => {
  test("通知先チャンネルを選んで保存すると、再読込後も選ばれている", async ({
    page,
  }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    const dialog = await openSettings(page);
    const select = dialog.getByRole("combobox", { name: CHANNEL_LABEL });
    await expect(select).toBeEnabled();
    await select.click();
    const listbox = page.getByRole("listbox");
    // カテゴリ名と Bot が投稿できるチャンネルが出て、投稿できないチャンネルは理由つきで選べない
    await expect(listbox).toContainText(E2E_CHANNELS.category.name);
    const staffOnly = listbox.getByRole("option", {
      name: `#${E2E_CHANNELS.staffOnly.name}`,
    });
    await expect(staffOnly).toHaveAttribute("aria-disabled", "true");
    await expect(staffOnly).toContainText("「チャンネルを見る」");
    // ボイスチャンネルは出ない
    await expect(
      listbox.getByRole("option", { name: `#${E2E_CHANNELS.voice.name}` }),
    ).toHaveCount(0);
    await listbox
      .getByRole("option", { name: `#${E2E_CHANNELS.notices.name}` })
      .click();
    await expect(select).toContainText(`#${E2E_CHANNELS.notices.name}`);
    await saveSettings(page, dialog);

    // 保存した通知先は API (GET /config) と再読込後のダイアログの両方に反映される
    const config = await page.request.get(configApi);
    expect(await config.json()).toMatchObject({
      notification_channel_id: E2E_CHANNELS.notices.id,
    });
    await page.reload();
    const reopened = await openSettings(page);
    await expect(
      reopened.getByRole("combobox", { name: CHANNEL_LABEL }),
    ).toContainText(`#${E2E_CHANNELS.notices.name}`);
    // 別のチャンネルにも変えられる (次の実行では最初の状態が違っても同じ手順で通る)
    await reopened.getByRole("combobox", { name: CHANNEL_LABEL }).click();
    await page
      .getByRole("listbox")
      .getByRole("option", { name: `#${E2E_CHANNELS.general.name}` })
      .click();
    await saveSettings(page, reopened);
    const changed = await page.request.get(configApi);
    expect(await changed.json()).toMatchObject({
      notification_channel_id: E2E_CHANNELS.general.id,
    });
  });

  test("API は Bot が投稿できないチャンネルや不正な通知設定を拒否する (表示だけの制御ではない)", async ({
    page,
  }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    const staffOnly = await page.request.put(configApi, {
      data: {
        restricted: false,
        notification_channel_id: E2E_CHANNELS.staffOnly.id,
      },
    });
    expect(staffOnly.status()).toBe(400);
    expect(await staffOnly.json()).toMatchObject({ error: "bad_request" });

    const voice = await page.request.put(configApi, {
      data: {
        restricted: false,
        notification_channel_id: E2E_CHANNELS.voice.id,
      },
    });
    expect(voice.status()).toBe(400);

    const tooMany = await page.request.put(configApi, {
      data: {
        restricted: false,
        default_notifications: Array.from({ length: 11 }, () => ({
          num: 1,
          unit: "hours",
        })),
      },
    });
    expect(tooMany.status()).toBe(400);

    const outOfRange = await page.request.put(configApi, {
      data: {
        restricted: false,
        default_notifications: [{ num: 0, unit: "minutes" }],
      },
    });
    expect(outOfRange.status()).toBe(400);
  });

  test("開始時刻の通知を止め、既定の事前通知を変えると新規作成の初期値に反映される", async ({
    page,
  }) => {
    await resetNotificationSettings(page);
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    const dialog = await openSettings(page);
    const notifyAtStart = dialog.getByRole("checkbox", {
      name: NOTIFY_AT_START_LABEL,
    });
    await expect(notifyAtStart).toBeChecked();
    await notifyAtStart.uncheck();
    // 既定の事前通知は 1 日前と 1 時間前から始まり、30 分前を足す
    const nums = dialog.getByLabel("通知のタイミング (数値)");
    await expect(nums).toHaveCount(2);
    await dialog.getByRole("button", { name: "通知を追加" }).click();
    await expect(nums).toHaveCount(3);
    await nums.nth(2).fill("30");
    await dialog.getByLabel("通知のタイミング (単位)").nth(2).click();
    await page.getByRole("option", { name: "分前" }).click();
    await saveSettings(page, dialog);

    const config = await page.request.get(configApi);
    expect(await config.json()).toMatchObject({
      notify_at_start: false,
      default_notifications: [
        { num: 1, unit: "days" },
        { num: 1, unit: "hours" },
        { num: 30, unit: "minutes" },
      ],
    });

    // 新規作成ダイアログの通知欄がサーバー既定になっている
    await page.reload();
    await page.getByRole("button", { name: "新規作成" }).click();
    const create = page.getByRole("dialog", { name: "予定を作成" });
    await expect(create).toBeVisible();
    const createNums = create.getByLabel("通知のタイミング (数値)");
    await expect(createNums).toHaveCount(3);
    await expect(createNums.nth(2)).toHaveValue("30");
    await expect(
      create.getByLabel("通知のタイミング (単位)").nth(2),
    ).toContainText("分前");
    await create.getByRole("button", { name: "キャンセル" }).click();

    // 再読込後の設定ダイアログにも残っている
    const reopened = await openSettings(page);
    await expect(
      reopened.getByRole("checkbox", { name: NOTIFY_AT_START_LABEL }),
    ).not.toBeChecked();
    await expect(reopened.getByLabel("通知のタイミング (数値)")).toHaveCount(3);
    await reopened.getByRole("button", { name: "キャンセル" }).click();

    // 他のテスト (予定の作成の既定の通知) のために元に戻す
    await resetNotificationSettings(page);
  });
});

test.describe("管理権限のないギルド (restricted)", () => {
  test("閲覧のみになり、設定も変更できない", async ({ page }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.member.id}`);
    await expect(page.getByText(E2E_GUILDS.member.name)).toBeVisible();
    await expect(page.getByText(RESTRICTED_NOTICE)).toBeVisible();
    await expect(page.getByRole("button", { name: "新規作成" })).toBeDisabled();

    const dialog = await openSettings(page);
    await expect(
      dialog.getByText("サーバーの設定の変更には", { exact: false }),
    ).toBeVisible();
    const restricted = dialog.getByRole("checkbox", { name: RESTRICTED_LABEL });
    await expect(restricted).toBeDisabled();
    await expect(restricted).toBeChecked();
    // 通知の設定 (#181) も見えるが変更できない。このギルドは通知先が未設定
    await expect(
      dialog.getByRole("checkbox", { name: NOTIFY_AT_START_LABEL }),
    ).toBeDisabled();
    await expect(
      dialog.getByRole("combobox", { name: CHANNEL_LABEL }),
    ).toBeDisabled();
    await expect(dialog.getByText("未設定 (通知は届きません)")).toBeVisible();
    // チャンネル一覧は本人に見えるものだけ (Bot には見える staff-only を一般メンバーには返さない)
    const channels = await page.request.get(
      `/local/api/guilds/${E2E_GUILDS.member.id}/channels`,
    );
    expect(channels.status()).toBe(200);
    const names = (await channels.json()).map((c: { name: string }) => c.name);
    expect(names).toContain(E2E_CHANNELS.general.name);
    expect(names).not.toContain(E2E_CHANNELS.staffOnly.name);
    await expect(
      dialog.getByRole("button", { name: "通知を追加" }),
    ).toBeDisabled();
    await expect(dialog.getByRole("button", { name: "保存" })).toBeDisabled();
  });

  test("API 側でも予定の作成と設定変更が拒否される (表示だけの制御ではない)", async ({
    page,
  }) => {
    // ブラウザと同じ cookie で直接 API を叩く
    const create = await page.request.post(
      `/local/api/events/${E2E_GUILDS.member.id}`,
      {
        data: {
          name: "restricted なのに作れてはいけない",
          description: null,
          notifications: [],
          color: "#F44336",
          is_all_day: false,
          start_at: "2026-09-01T10:00:00",
          end_at: "2026-09-01T11:00:00",
        },
      },
    );
    expect(create.status()).toBe(403);
    expect(await create.json()).toMatchObject({ error: "forbidden" });

    const config = await page.request.put(
      `/local/api/guilds/${E2E_GUILDS.member.id}/config`,
      { data: { restricted: false } },
    );
    expect(config.status()).toBe(403);
  });
});
