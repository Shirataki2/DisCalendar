import { expect, test } from "@playwright/test";
import { createEvent, openEventPopover } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

const guild = E2E_GUILDS.admin.id;
test.beforeEach(async ({ page }) => {
  await page.goto(`/dashboard/${guild}`);
  await page.getByRole("button", { name: "新規作成" }).click();
});

for (const close of ["Escape", "Close", "キャンセル"]) {
  test(`${close}: 未変更なら閉じ、変更時は続行・破棄を選べる`, async ({
    page,
  }) => {
    const form = page.getByRole("dialog", { name: "予定を作成" });
    const closeForm = async () => {
      if (close === "Escape") await page.keyboard.press("Escape");
      else await form.getByRole("button", { name: close, exact: true }).click();
    };
    await closeForm();
    await expect(form).toBeHidden();
    await page.getByRole("button", { name: "新規作成" }).click();
    await form.getByLabel("タイトル").fill("未保存の予定");
    await form.getByLabel("開始日", { exact: true }).click();
    await page.locator('[data-selected-single="true"]').press("ArrowRight");
    await page.keyboard.press("Enter");
    const startDate = await form
      .getByLabel("開始日", { exact: true })
      .textContent();
    await form.getByRole("checkbox", { name: "終日", exact: true }).check();
    await closeForm();
    await page.getByRole("button", { name: "編集を続ける" }).click();
    await expect(form.getByLabel("タイトル")).toHaveValue("未保存の予定");
    await expect(form.getByLabel("開始日", { exact: true })).toHaveText(
      startDate ?? "",
    );
    await expect(
      form.getByRole("checkbox", { name: "終日", exact: true }),
    ).toBeChecked();
    await closeForm();
    await page.getByRole("button", { name: "破棄して閉じる" }).click();
    await expect(form).toBeHidden();
    await page.getByRole("button", { name: "新規作成" }).click();
    await expect(form.getByLabel("タイトル")).toHaveValue("");
    await expect(
      form.getByRole("checkbox", { name: "終日", exact: true }),
    ).not.toBeChecked();
  });
}

test("変更を元に戻せば確認せず閉じ、背景クリックでは閉じない", async ({
  page,
}) => {
  const form = page.getByRole("dialog", { name: "予定を作成" });
  await form.getByLabel("タイトル").fill("一時入力");
  await page.mouse.click(5, 5);
  await expect(form).toBeVisible();
  await expect(page.getByRole("alertdialog")).toBeHidden();
  await form.getByLabel("タイトル").fill("");
  await page.keyboard.press("Escape");
  await expect(form).toBeHidden();
});

test("添付だけの変更と保存失敗でも、編集を続けると入力を保持する", async ({
  page,
}) => {
  const form = page.getByRole("dialog", { name: "予定を作成" });
  const picker = form.getByLabel("ファイルを添付", { exact: true });
  await expect(picker).toBeEnabled();
  await picker.setInputFiles({
    name: "資料.pdf",
    mimeType: "application/pdf",
    buffer: Buffer.from("%PDF-1.7\n%%EOF"),
  });
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "編集を続ける" }).click();
  await expect(form.getByText("資料.pdf", { exact: false })).toBeVisible();
  await form.getByLabel("タイトル").fill("保存失敗の予定");
  await page.route(`**/local/api/events/${guild}`, async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    await route.fulfill({ status: 500, json: { error: "internal_error" } });
  });
  await form.getByRole("button", { name: "作成", exact: true }).click();
  await expect(form.getByRole("alert")).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "編集を続ける" }).click();
  await expect(form.getByLabel("タイトル")).toHaveValue("保存失敗の予定");
  await expect(form.getByText("資料.pdf", { exact: false })).toBeVisible();
});

test("編集・複製は元の値を基準に変更を検出し、保存成功時は確認しない", async ({
  page,
}) => {
  await page.keyboard.press("Escape");
  const title = `破棄確認 ${Date.now()}`;
  await createEvent(page, title);
  await expect(page.getByRole("dialog")).toBeHidden();
  for (const mode of ["編集", "複製"]) {
    const open = async () => {
      const popover = await openEventPopover(page, title);
      await popover.getByRole("button", { name: mode, exact: true }).click();
    };
    await open();
    const form = page.getByRole("dialog", {
      name: mode === "編集" ? "予定を編集" : "予定を作成",
    });
    await page.keyboard.press("Escape");
    await expect(form).toBeHidden();
    await open();
    await form.getByLabel("説明").fill("未保存の説明");
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "編集を続ける" }).click();
    await expect(form.getByLabel("説明")).toHaveValue("未保存の説明");
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "破棄して閉じる" }).click();
    await expect(form).toBeHidden();
  }
});

test("追加前のメンションIDも保持し、空に戻すと確認せず閉じる", async ({
  page,
}) => {
  const form = page.getByRole("dialog", { name: "予定を作成" });
  const input = form.getByLabel("メンションするユーザーID");
  await input.fill("100000000000000001");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "編集を続ける" }).click();
  await expect(input).toHaveValue("100000000000000001");
  await input.fill("");
  await page.keyboard.press("Escape");
  await expect(form).toBeHidden();
});
