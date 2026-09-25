import { expect, test } from "@playwright/test";
import type { ApiEvent } from "../src/lib/api/types";
import { dayCell, dragEventTo, eventOn } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

const guild = E2E_GUILDS.admin.id;
const base = `/local/api/events/${guild}`;
for (const width of [320, 1280]) {
  test(`${width}px: 繰り返し設定を適用し、各回と編集範囲を確認する`, async ({
    page,
  }) => {
    await page.clock.setFixedTime(new Date("2026-09-21T03:00:00Z"));
    await page.setViewportSize({ width, height: 900 });
    await page.goto(`/dashboard/${guild}`);
    await page.getByRole("button", { name: "新規作成", exact: true }).click();
    await page.addStyleTag({
      content: "nextjs-portal { display: none !important; }",
    });
    let form = page.getByRole("dialog", { name: "予定を作成", exact: true });
    const name = `繰り返し ${width}`;
    await form.getByLabel("タイトル").fill(name);
    await form.getByRole("button", { name: "繰り返しなし" }).click();
    const settings = page.getByRole("dialog", {
      name: "繰り返しの設定",
      exact: true,
    });
    expect(
      await settings
        .getByRole("button", { name: "設定を適用" })
        .evaluate(
          (element) => element.parentElement?.getBoundingClientRect().bottom,
        ),
    ).toBe(
      await settings.evaluate(
        (element) => element.getBoundingClientRect().bottom,
      ),
    );
    await settings.getByLabel("繰り返しの頻度").selectOption("weekly");
    await expect(
      settings.getByRole("listitem").filter({ hasText: "2026/09/21" }),
    ).toBeVisible();
    const settingsHeight = await settings.evaluate(
      (element) => element.getBoundingClientRect().height,
    );
    const expectStableHeight = async () => {
      expect(
        await settings.evaluate(
          (element) => element.getBoundingClientRect().height,
        ),
      ).toBe(settingsHeight);
    };
    await settings.getByRole("radio", { name: "終了日", exact: true }).check();
    await expectStableHeight();
    await settings.getByLabel("回数", { exact: true }).check();
    await expectStableHeight();
    await settings.getByLabel("繰り返し回数").fill("3");
    await settings.getByRole("button", { name: "火", exact: true }).click();
    await expectStableHeight();
    await settings.getByRole("button", { name: "火", exact: true }).click();
    await expectStableHeight();
    await expect(
      settings.getByRole("listitem").filter({ hasText: "2026/09/21" }),
    ).toBeVisible();
    await expect(
      settings.getByRole("button", { name: "設定を適用" }),
    ).toBeEnabled();
    expect(
      await settings.evaluate((el) => el.scrollWidth <= el.clientWidth),
    ).toBe(true);
    await page.screenshot({ path: `/tmp/recurrence-settings-${width}.png` });
    await settings.getByRole("button", { name: "設定を適用" }).click();
    form = page.getByRole("dialog", { name: "予定を作成", exact: true });
    await expect(
      form.getByRole("button", { name: "毎週月曜日／全3回" }),
    ).toBeFocused();
    const response = page.waitForResponse(
      (r) => r.url().endsWith(base) && r.request().method() === "POST",
    );
    await form.getByRole("button", { name: "作成", exact: true }).click();
    const created = await response;
    expect(created.status()).toBe(201);
    await expect(form).toBeHidden();
    let rows: ApiEvent[] = await (
      await page.request.get(
        `${base}?start=2026-09-01T00:00:00&end=2026-11-01T00:00:00`,
      )
    ).json();
    rows = rows.filter((e) => e.name === name);
    expect(rows).toHaveLength(3);
    expect(rows.map((e) => e.start_at.slice(0, 10))).toEqual([
      "2026-09-21",
      "2026-09-28",
      "2026-10-05",
    ]);
    await page
      .getByRole("gridcell")
      .filter({
        has: page.getByRole("button", { name: new RegExp(`${name}$`) }),
      })
      .first()
      .getByRole("button", { name: new RegExp(`${name}$`) })
      .click();
    await page.getByRole("button", { name: "編集", exact: true }).click();
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "この回以降", exact: true })
      .click();
    const edit = page.getByRole("dialog", { name: "予定を編集", exact: true });
    await edit.getByLabel("タイトル").fill(`${name}変更`);
    await edit.getByRole("button", { name: "保存", exact: true }).click();
    await expect(edit).toBeHidden();
    const changed: ApiEvent[] = await (
      await page.request.get(
        `${base}?start=2026-09-01T00:00:00&end=2026-11-01T00:00:00`,
      )
    ).json();
    expect(
      changed.filter((e) => e.name === `${name}変更`).map((e) => e.id),
    ).toEqual(rows.map((e) => e.id));
    const first = changed.find((e) => e.id === rows[0].id);
    expect(first?.recurrence).toBeTruthy();
    const removed = await page.request.delete(
      `${base}/${first?.id}?scope=future&expected_series_version=${first?.recurrence?.version}`,
    );
    expect(removed.status()).toBe(204);
  });
}

test("不正な曜日とDiscord連携をAPI境界で拒否する", async ({ page }) => {
  const invalid = await page.request.post(`${base}/recurrence/preview`, {
    data: {
      start_at: "2026-09-21T10:00:00",
      recurrence: {
        frequency: "weekly",
        weekdays: [2],
        end: { type: "never" },
      },
    },
  });
  expect(invalid.status()).toBe(400);
  const linked = await page.request.post(base, {
    data: {
      name: "拒否される予定",
      color: "#2196F3",
      start_at: "2030-01-01T10:00:00",
      end_at: "2030-01-01T11:00:00",
      discord_scheduled_event: true,
      recurrence_rule: { frequency: "daily", end: { type: "never" } },
    },
  });
  expect(linked.status()).toBe(400);
  const denied = await page.request.post(
    `/local/api/events/${E2E_GUILDS.member.id}/recurrence/preview`,
    {
      data: {
        start_at: "2026-09-21T10:00:00",
        recurrence: { frequency: "daily", end: { type: "never" } },
      },
    },
  );
  expect(denied.status()).toBe(403);
});

test("繰り返し解除を保存するまではDiscord連携を表示しない", async ({
  page,
}) => {
  await page.clock.setFixedTime(new Date("2026-09-21T03:00:00Z"));
  const title = "繰り返し解除の連携";
  const created = await page.request.post(base, {
    data: {
      name: title,
      color: "#2196F3",
      start_at: "2026-09-21T19:30:00",
      end_at: "2026-09-21T20:30:00",
      recurrence_rule: {
        frequency: "weekly",
        weekdays: [0],
        end: { type: "count", count: 2 },
      },
    },
  });
  expect(created.status()).toBe(201);
  const event = (await created.json()) as ApiEvent;
  await page.goto(`/dashboard/${guild}`);
  await eventOn(page, title).first().click();
  await page.getByRole("button", { name: "編集", exact: true }).click();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "この回以降", exact: true })
    .click();
  const form = page.getByRole("dialog", { name: "予定を編集", exact: true });
  await form.getByRole("button", { name: "毎週月曜日／全2回" }).click();
  const settings = page.getByRole("dialog", { name: "繰り返しの設定" });
  await settings.getByLabel("繰り返しの頻度").selectOption("none");
  await settings.getByRole("button", { name: "設定を適用" }).click();
  await expect(
    form.getByRole("checkbox", {
      name: "Discord のイベントとしても作成する",
    }),
  ).toHaveCount(0);
  await form.getByRole("button", { name: "キャンセル" }).click();
  await page
    .getByRole("alertdialog", { name: "未保存の変更を破棄しますか？" })
    .getByRole("button", { name: "破棄して閉じる" })
    .click();
  const removed = await page.request.delete(
    `${base}/${event.id}?scope=future&expected_series_version=${event.recurrence?.version}`,
  );
  expect(removed.status()).toBe(204);
});

test("設定の取消・キーボード・開始日の不一致と未保存確認", async ({ page }) => {
  await page.clock.setFixedTime(new Date("2026-09-21T03:00:00Z"));
  await page.goto(`/dashboard/${guild}`);
  await page.addStyleTag({
    content: "nextjs-portal { display: none !important; }",
  });
  await page.getByRole("button", { name: "新規作成", exact: true }).click();
  const form = page.getByRole("dialog", { name: "予定を作成", exact: true });
  await form.getByRole("button", { name: "繰り返しなし" }).focus();
  await page.keyboard.press("Enter");
  const settings = page.getByRole("dialog", { name: "繰り返しの設定" });
  await expect(settings.getByRole("heading")).toBeFocused();
  await settings.getByLabel("繰り返しの頻度").selectOption("weekly");
  await settings.getByRole("button", { name: "月", exact: true }).click();
  await settings.getByRole("button", { name: "火", exact: true }).click();
  await expect(settings.getByRole("alert")).toContainText("開始日の曜日");
  await expect(
    settings.getByRole("button", { name: "設定を適用" }),
  ).toBeDisabled();
  await settings
    .getByRole("button", { name: "予定に戻る", exact: false })
    .focus();
  await page.keyboard.press("Enter");
  await expect(
    form.getByRole("button", { name: "繰り返しなし" }),
  ).toBeFocused();
  await form.getByRole("button", { name: "繰り返しなし" }).click();
  await settings.getByLabel("繰り返しの頻度").selectOption("monthly_weekday");
  await expect(settings.getByLabel("週の順番")).toHaveValue("3");
  await settings
    .getByRole("button", { name: "キャンセル", exact: true })
    .click();
  await form.getByRole("button", { name: "繰り返しなし" }).click();
  await settings.getByLabel("繰り返しの頻度").selectOption("daily");
  await expect(
    settings.getByRole("button", { name: "設定を適用" }),
  ).toBeEnabled();
  await settings.getByRole("heading").focus();
  await page.keyboard.press("Escape");
  const discard = page.getByRole("alertdialog", {
    name: "未保存の変更を破棄しますか？",
  });
  await expect(discard).toBeVisible();
  await discard.getByRole("button", { name: "編集を続ける" }).click();
  await expect(settings).toBeVisible();
  await settings.getByRole("button", { name: "設定を適用" }).click();
  await page.keyboard.press("Escape");
  await discard.getByRole("button", { name: "破棄して閉じる" }).click();
  await expect(form).toBeHidden();
});

test("繰り返しのドラッグは取消・保存失敗で元に戻り、以降の削除を確認できる", async ({
  page,
}) => {
  await page.clock.setFixedTime(new Date("2026-09-21T03:00:00Z"));
  await page.setViewportSize({ width: 1280, height: 1000 });
  const response = await page.request.post(base, {
    data: {
      name: `定例の移動 ${Date.now()}`,
      color: "#2196F3",
      start_at: "2026-09-21T13:00:00",
      end_at: "2026-09-21T14:00:00",
      recurrence_rule: {
        frequency: "weekly",
        weekdays: [0],
        end: { type: "count", count: 2 },
      },
    },
  });
  expect(response.status()).toBe(201);
  const original: ApiEvent = await response.json();
  await page.goto(`/dashboard/${guild}`);
  await page.addStyleTag({
    content: "nextjs-portal { display: none !important; }",
  });
  const source = dayCell(page, new Date(2026, 8, 21));
  const destination = dayCell(page, new Date(2026, 8, 22));
  await expect(eventOn(source, original.name)).toBeVisible();
  const drag = async () => {
    await expect(page.getByRole("alertdialog")).toBeHidden();
    await destination.scrollIntoViewIfNeeded();
    // 閉じるアニメーション中のダイアログが次のドラッグを遮らないようにする。
    await eventOn(source, original.name).click({ trial: true });
    await dragEventTo(page, eventOn(source, original.name), destination);
    await expect(
      page.getByRole("alertdialog", { name: "変更する予定" }),
    ).toBeVisible();
  };
  await drag();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "キャンセル" })
    .click();
  await expect(eventOn(source, original.name)).toBeVisible();
  await expect(eventOn(destination, original.name)).toHaveCount(0);
  await page.route(`**${base}/${original.id}`, (route) =>
    route.request().method() === "PUT"
      ? route.fulfill({
          status: 409,
          contentType: "application/json",
          body: JSON.stringify({ error: "conflict", message: "changed" }),
        })
      : route.continue(),
  );
  await drag();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "この回以降", exact: true })
    .click();
  await expect(
    page.getByRole("alert").filter({ hasText: "保存できません" }),
  ).toBeVisible();
  await expect(eventOn(source, original.name)).toBeVisible();
  await expect(eventOn(destination, original.name)).toHaveCount(0);
  await page.unroute(`**${base}/${original.id}`);
  await eventOn(source, original.name).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "削除", exact: true })
    .click();
  const deletion = page.getByRole("alertdialog");
  await deletion.getByLabel("この回以降", { exact: true }).check();
  await expect(deletion).toContainText(
    "個別編集済みの回と添付ファイルも含めて削除",
  );
  await deletion.getByRole("button", { name: "削除", exact: true }).click();
  await expect(eventOn(page, original.name)).toHaveCount(0);
});
