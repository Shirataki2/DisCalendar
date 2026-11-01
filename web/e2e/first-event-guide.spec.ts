import { expect, test } from "@playwright/test";
import { eventOn } from "./calendar";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS } from "./fixtures";
import { deleteEventsNamed, deleteGuildEvents } from "./seed";

// 予定がまだないサーバーで、最初の予定作りを手助けする案内 (#377)。
// 予定の有無はサーバー単位なので、対象のサーバーの予定を空にしてから確かめる

const GUIDE_TITLE = "このサーバーにはまだ予定がありません";
const title = `E2E 最初の予定 ${Date.now().toString(36)}`;

test.afterAll(async () => {
  await deleteEventsNamed(DATABASE_URL, [title]);
});

test("予定がなければ案内から最初の予定を作れ、作ると案内が消える", async ({
  page,
}) => {
  // restricted ではないので一般メンバーでも予定を作れるサーバー
  const guildId = E2E_GUILDS.noUserEventsPerm.id;
  await deleteGuildEvents(DATABASE_URL, guildId);
  await page.goto(`/dashboard/${guildId}`);

  const guide = page.getByRole("region", { name: GUIDE_TITLE });
  await expect(guide).toBeVisible();

  // 補足は折りたたんであり、通知先の設定はサーバー設定ダイアログで行う
  const settingsButton = guide.getByRole("button", {
    name: "通知先を設定する",
  });
  await expect(settingsButton).toBeHidden();
  await guide.getByText("あわせて設定しておくと便利です").click();
  await settingsButton.click();
  const settings = page.getByRole("dialog", { name: "サーバー設定" });
  await expect(settings).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(settings).toBeHidden();

  await guide.getByRole("button", { name: "最初の予定を作る" }).click();
  const dialog = page.getByRole("dialog", { name: "予定を作成" });
  await expect(dialog).toBeVisible();
  await dialog.getByLabel("タイトル").fill(title);
  await dialog.getByRole("button", { name: "作成" }).click();
  await expect(dialog).toBeHidden();
  await expect(eventOn(page, title)).toBeVisible();

  // 予定の有無を取り直して、案内を出さなくなる (再読み込みしても出ない)
  await expect(guide).toBeHidden();
  await page.reload();
  await expect(eventOn(page, title)).toBeVisible();
  await expect(page.getByRole("region", { name: GUIDE_TITLE })).toHaveCount(0);
});

test("予定を編集できない人には管理者に頼むよう案内する", async ({ page }) => {
  // restricted 設定済みで、テストユーザーは一般メンバー
  const guildId = E2E_GUILDS.member.id;
  await deleteGuildEvents(DATABASE_URL, guildId);
  await page.goto(`/dashboard/${guildId}`);

  const guide = page.getByRole("region", { name: GUIDE_TITLE });
  await expect(guide).toBeVisible();
  await expect(guide).toContainText("サーバーの管理者に頼んでください");
  await expect(
    guide.getByRole("button", { name: "最初の予定を作る" }),
  ).toHaveCount(0);

  // 閉じると、そのサーバーでは再読み込みしても出さない
  await guide.getByRole("button", { name: "最初の予定の案内を閉じる" }).click();
  await expect(guide).toBeHidden();
  await page.reload();
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(page.getByRole("region", { name: GUIDE_TITLE })).toHaveCount(0);
});
