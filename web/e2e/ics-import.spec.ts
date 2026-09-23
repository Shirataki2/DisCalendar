import { expect, test } from "@playwright/test";
import type { ApiEvent, ImportEventInput } from "../src/lib/api/types";
import { eventOn } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

const guild = E2E_GUILDS.admin.id;
const base = `/local/api/events/${guild}`;
const stamp = Date.now().toString(36);
const duplicateTitle = `ICS 重複 ${stamp}`;
const importedTitle = `ICS 取込 ${stamp}`;

function icsEvent(uid: string, title: string, start: string) {
  return `BEGIN:VEVENT\r\nUID:${uid}\r\nDTSTAMP:20260901T000000Z\r\nDTSTART;TZID=Asia/Tokyo:${start}\r\nDTEND;TZID=Asia/Tokyo:${start.slice(0, 9)}110000\r\nSUMMARY:${title}\r\nEND:VEVENT\r\n`;
}

test("390pxでプレビューし、重複を選び直して共通色で取り込める", async ({
  page,
}) => {
  await page.clock.setFixedTime(new Date("2026-09-21T03:00:00Z"));
  await page.setViewportSize({ width: 390, height: 844 });
  await page.addInitScript(() => {
    localStorage.setItem(
      "discalendar-calendar-settings",
      JSON.stringify({ defaultColor: "#3F51B5" }),
    );
  });
  const duplicate = await page.request.post(base, {
    data: {
      name: duplicateTitle,
      color: "#2196F3",
      start_at: "2026-09-22T10:00:00",
      end_at: "2026-09-22T11:00:00",
    },
  });
  expect(duplicate.status()).toBe(201);

  await page.goto(`/dashboard/${guild}`);
  await page.getByRole("button", { name: "新規作成" }).click();
  await page.getByRole("menuitem", { name: "ICSファイルから取り込む" }).click();
  const dialog = page.getByRole("dialog", {
    name: "ICSファイルから取り込む",
  });
  const height = await dialog.evaluate((element) => element.clientHeight);
  const ics = `BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//E2E//JA\r\n${icsEvent("duplicate", duplicateTitle, "20260922T100000")}${icsEvent("new", importedTitle, "20260922T100000")}${icsEvent("filtered", `ICS 期間外 ${stamp}`, "20260930T100000")}END:VCALENDAR\r\n`;
  await dialog.getByLabel("ICSファイル").setInputFiles({
    name: "events.ics",
    mimeType: "text/calendar",
    buffer: Buffer.from(ics),
  });
  await expect(dialog.getByLabel("取り込み色")).toContainText("#3F51B5");

  const duplicateRow = dialog
    .getByRole("listitem")
    .filter({ hasText: duplicateTitle });
  await expect(duplicateRow).toContainText("すでにあります");
  await expect(duplicateRow.getByRole("checkbox")).not.toBeChecked();
  await expect(
    dialog.getByRole("listitem").filter({ hasText: importedTitle }),
  ).toBeVisible();

  await dialog.getByLabel("終了日").fill("2026-09-22");
  await dialog.getByRole("button", { name: "期間を反映" }).click();
  await expect(dialog.getByText("2件中 1件を選択")).toBeVisible();
  await dialog.getByText("すべて選択", { exact: true }).click();
  await expect(dialog.getByText("2件中 2件を選択")).toBeVisible();
  await dialog.getByLabel("取り込み色").click();
  await page.getByRole("button", { name: "#009688" }).click();
  expect(await dialog.evaluate((element) => element.clientHeight)).toBe(height);
  expect(
    await dialog.evaluate(
      (element) => element.scrollWidth <= element.clientWidth,
    ),
  ).toBe(true);

  const imported = page.waitForResponse(
    (response) =>
      response.url().endsWith(`${base}/bulk`) &&
      response.request().method() === "POST",
  );
  await dialog.getByRole("button", { name: "2件を取り込む" }).click();
  expect((await imported).status()).toBe(201);
  await expect(dialog.getByText("2件の予定を取り込みました。")).toBeVisible();
  await dialog.getByRole("button", { name: "閉じる" }).click();
  await expect(eventOn(page, importedTitle)).toBeVisible();

  const rows: ApiEvent[] = await (
    await page.request.get(
      `${base}?start=2026-09-22T00:00:00&end=2026-09-23T00:00:00`,
    )
  ).json();
  const importedRows = rows.filter((event) =>
    [duplicateTitle, importedTitle].includes(event.name),
  );
  expect(importedRows).toHaveLength(3);
  expect(
    importedRows.filter((event) => event.color === "#009688"),
  ).toHaveLength(2);
  expect(importedRows.every((event) => event.notifications.length === 0)).toBe(
    true,
  );
  expect(
    importedRows.every((event) => event.discord_scheduled_event_id === null),
  ).toBe(true);

  for (const event of importedRows) {
    expect((await page.request.delete(`${base}/${event.id}`)).status()).toBe(
      204,
    );
  }
});

test("権限と一括検証で、不正な2件目があると1件も作らない", async ({ page }) => {
  const denied = await page.request.post(
    `/local/api/events/${E2E_GUILDS.member.id}/import/preview`,
    { data: { ics: "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n" } },
  );
  expect(denied.status()).toBe(403);

  const name = `ICS rollback ${stamp}`;
  const valid: ImportEventInput = {
    name,
    description: null,
    color: "#2196F3",
    is_all_day: false,
    start_at: "2026-09-22T10:00:00",
    end_at: "2026-09-22T11:00:00",
  };
  const response = await page.request.post(`${base}/bulk`, {
    data: { events: [valid, { ...valid, name: "" }] },
  });
  expect(response.status()).toBe(400);
  expect(await response.text()).toContain("events[2]");
  const rows: ApiEvent[] = await (
    await page.request.get(
      `${base}?start=2026-09-22T00:00:00&end=2026-09-23T00:00:00`,
    )
  ).json();
  expect(rows.filter((event) => event.name === name)).toHaveLength(0);
});

test("200件と初期10,000回の上限を超える取り込みを拒否する", async ({
  page,
}) => {
  const events = Array.from({ length: 201 }, (_, index) =>
    icsEvent(`limit-${index}`, `ICS 上限 ${index}`, "20260922T100000"),
  ).join("");
  const preview = await page.request.post(`${base}/import/preview`, {
    data: {
      ics: `BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//E2E//JA\r\n${events}END:VCALENDAR\r\n`,
    },
  });
  expect(preview.status()).toBe(200);
  expect(await preview.json()).toMatchObject({
    matched_count: 201,
    over_event_limit: true,
  });

  const recurring: ImportEventInput = {
    name: "ICS 繰り返し上限",
    description: null,
    color: "#2196F3",
    is_all_day: false,
    start_at: "2026-09-22T10:00:00",
    end_at: "2026-09-22T11:00:00",
    recurrence_rule: { frequency: "daily", end: { type: "never" } },
  };
  const bulk = await page.request.post(`${base}/bulk`, {
    data: { events: Array.from({ length: 14 }, () => recurring) },
  });
  expect(bulk.status()).toBe(400);
  expect(await bulk.text()).toContain("10000");
});
