import { expect, type Page, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

// サーバー設定 (restricted モード) の切替と、管理権限のないユーザーの表示。
// admin ギルドではテストユーザーがオーナー、member ギルドでは権限のない一般メンバー (Discord モックの定義)

const RESTRICTED_NOTICE =
  "このサーバーでは管理権限を持つユーザーのみ予定を編集できます";

async function openSettings(page: Page) {
  await page.getByRole("button", { name: "サーバー設定" }).click();
  const dialog = page.getByRole("dialog", { name: "サーバー設定" });
  await expect(dialog).toBeVisible();
  return dialog;
}

test.describe("管理権限のあるギルド", () => {
  test.describe.configure({ mode: "serial" });

  test("restricted を有効にして保存すると、再読込後も有効で自分は編集できる", async ({
    page,
  }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    await expect(page.getByText(RESTRICTED_NOTICE)).toHaveCount(0);

    const dialog = await openSettings(page);
    const checkbox = dialog.getByRole("checkbox");
    await expect(checkbox).toBeEnabled();
    await expect(checkbox).not.toBeChecked();
    await checkbox.check();
    const saved = page.waitForResponse(
      (res) =>
        res.url().includes(`/local/api/guilds/${E2E_GUILDS.admin.id}/config`) &&
        res.request().method() === "PUT",
    );
    await dialog.getByRole("button", { name: "保存" }).click();
    expect((await saved).status()).toBe(200);
    await expect(dialog).toBeHidden();

    await page.reload();
    const reopened = await openSettings(page);
    await expect(reopened.getByRole("checkbox")).toBeChecked();
    await reopened.getByRole("button", { name: "キャンセル" }).click();
    // 管理権限があるので restricted でも編集できる (注意書きも出ない)
    await expect(page.getByRole("button", { name: "新規作成" })).toBeEnabled();
    await expect(page.getByText(RESTRICTED_NOTICE)).toHaveCount(0);
  });

  test("restricted を無効に戻せる", async ({ page }) => {
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    const dialog = await openSettings(page);
    const checkbox = dialog.getByRole("checkbox");
    await expect(checkbox).toBeChecked();
    await checkbox.uncheck();
    await dialog.getByRole("button", { name: "保存" }).click();
    await expect(dialog).toBeHidden();

    await page.reload();
    const reopened = await openSettings(page);
    await expect(reopened.getByRole("checkbox")).not.toBeChecked();
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
    await expect(dialog.getByRole("checkbox")).toBeDisabled();
    await expect(dialog.getByRole("checkbox")).toBeChecked();
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
