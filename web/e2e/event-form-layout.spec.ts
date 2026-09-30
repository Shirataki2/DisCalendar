import { expect, test } from "@playwright/test";
import { openEventTab } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

for (const width of [390, 1280]) {
  test(`${width}px: 保存操作を固定し、別タブのエラーへ移動する`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 844 });
    await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
    await page.getByRole("button", { name: "新規作成", exact: true }).click();
    const form = page.getByRole("dialog", { name: "予定を作成" });
    await expect(
      form.getByText(`保存先: ${E2E_GUILDS.admin.name}`),
    ).toBeVisible();
    const save = form.getByRole("button", { name: "作成", exact: true });
    const header = form.getByRole("heading", { name: "予定を作成" });
    const close = form.getByRole("button", { name: "Close", exact: true });
    const cancel = form.getByRole("button", {
      name: "キャンセル",
      exact: true,
    });
    for (const target of [save, header, close, cancel])
      await expect(target).toBeInViewport();
    await form.getByLabel("タイトル").fill("配置の確認");
    await openEventTab(form, "繰り返し・通知");
    const heading = form.getByRole("heading", { name: /^事前通知/ });
    await expect(heading).toContainText("1日前、1時間前");
    const number = form.getByLabel("通知のタイミング (数値)").first();
    await number.fill("0");
    // 基本タブから保存しても、エラーのある通知タブへ切り替えてフォーカスする。
    await openEventTab(form, "基本");
    await expect(number).toBeHidden();
    await save.click();
    const notifyTab = form.getByRole("tab", { name: /^繰り返し・通知/ });
    await expect(notifyTab).toHaveAttribute("aria-selected", "true");
    await expect(notifyTab).toContainText("入力エラーあり");
    await expect(number).toBeVisible();
    await expect(number).toBeFocused();
    await expect(number).toHaveAttribute("aria-invalid", "true");
    await expect(number).toBeInViewport();
    await number.fill("2");
    await expect(heading).toContainText("2日前");
    const before = await save.boundingBox();
    await form.getByTestId("event-form-fields").evaluate((element) => {
      element.scrollTop = element.scrollHeight;
    });
    for (const target of [save, header, close, cancel])
      await expect(target).toBeInViewport();
    expect((await save.boundingBox())?.y).toBe(before?.y);
    expect(
      await form.evaluate(
        (element) => element.scrollWidth <= element.clientWidth,
      ),
    ).toBe(true);
    await openEventTab(form, "添付・共有");
    await expect(
      form.getByText("1件 約10.5 MB", { exact: false }),
    ).toBeVisible();
    await form
      .locator("summary")
      .filter({ hasText: "容量制限とサーバー使用量" })
      .click();
    await expect(
      form.getByText("10,485,760 バイト", { exact: false }),
    ).toBeVisible();
    await page.screenshot({ path: `/tmp/event-form-${width}.png` });
    await openEventTab(form, "繰り返し・通知");
    const userId = form.getByLabel("メンションするユーザーID");
    await userId.fill("abc");
    await form
      .getByRole("button", { name: "ユーザーを追加", exact: true })
      .click();
    await expect(userId).toBeFocused();
    await expect(
      form.getByText("正しいユーザーIDを入力してください"),
    ).toBeVisible();
    await userId.fill("");
    await cancel.click();
    await expect(page.getByRole("alertdialog")).toBeVisible();
    await page.getByRole("button", { name: "編集を続ける" }).click();
    await expect(form.getByLabel("タイトル")).toHaveValue("配置の確認");
  });
}
