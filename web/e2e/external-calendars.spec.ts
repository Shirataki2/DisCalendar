import { createServer } from "node:http";
import { expect, test } from "@playwright/test";
import { eventOn } from "./calendar";
import { E2E_GUILDS } from "./fixtures";

test("外部 ICS の重ね表示・再取得・失敗表示・権限境界", async ({ page }) => {
  const title = `E2E 外部予定 ${Date.now()}`;
  const requests: (string | undefined)[] = [];
  let failedRequests = 0;
  let receiverPort = 0;
  const receiver = createServer((req, res) => {
    if (req.url === "/private-redirect.ics") {
      res
        .writeHead(302, {
          location: `http://127.0.0.1:${receiverPort}/calendar.ics`,
        })
        .end();
      return;
    }
    if (req.url === "/calendar.ics") {
      requests.push(req.headers["if-none-match"]);
      if (req.headers["if-none-match"] === '"sample-1"') {
        res.writeHead(304).end();
        return;
      }
      res.writeHead(200, {
        etag: '"sample-1"',
        "content-type": "application/octet-stream",
      });
      res.end(
        `BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:external-e2e\r\nDTSTAMP:20260101T000000Z\r\nDTSTART:20261001T100000\r\nDTEND:20261001T110000\r\nSUMMARY:${title}\r\nDESCRIPTION:外部から取得した説明\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n`,
      );
      return;
    }
    if (req.url === "/exception.ics") {
      res.writeHead(200, { "content-type": "text/calendar" });
      res.end(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:exception-e2e\r\nDTSTAMP:20260101T000000Z\r\nDTSTART:20261001T100000\r\nRRULE:FREQ=DAILY;COUNT=3\r\nEXDATE:20261002T100000\r\nSUMMARY:例外付き\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
      );
      return;
    }
    failedRequests++;
    res.writeHead(500).end("失敗した URL や本文は画面に出さない");
  });
  await new Promise<void>((resolve) =>
    receiver.listen(0, "127.0.0.1", resolve),
  );
  const address = receiver.address();
  if (!address || typeof address === "string")
    throw new Error("ICS モックの起動に失敗");
  receiverPort = address.port;
  const guildId = E2E_GUILDS.admin.id;
  const base = `/local/api/guilds/${guildId}/external-calendars`;
  const created: number[] = [];
  try {
    await page.clock.setFixedTime(new Date("2026-10-01T12:00:00+09:00"));
    await page.goto(`/dashboard/${guildId}?date=2026-10-01`);
    await page.getByRole("button", { name: "サーバー設定" }).click();
    const dialog = page.getByRole("dialog", { name: "サーバー設定" });
    const section = dialog.getByRole("region", {
      name: "外部カレンダーを重ねて表示する",
    });
    await section.getByLabel("表示名").fill("大会日程");
    await section
      .getByLabel("ICS URL")
      .fill(`http://webhook.test:${address.port}/calendar.ics`);
    const added = page.waitForResponse(
      (response) =>
        response.url().includes(base) && response.request().method() === "POST",
    );
    await section.getByRole("button", { name: "追加" }).click();
    expect((await added).status()).toBe(201);
    const first = (await (await page.request.get(base)).json()).find(
      (item: { name: string }) => item.name === "大会日程",
    );
    created.push(first.id);
    await page.keyboard.press("Escape");
    await expect(eventOn(page, title)).toBeVisible();
    const chip = page
      .getByRole("list", { name: "外部カレンダーの凡例" })
      .getByRole("button", { name: "大会日程" });
    await chip.click();
    await expect(eventOn(page, title)).toHaveCount(0);
    await chip.click();
    await eventOn(page, title).click();
    const popover = page.getByRole("dialog").filter({ hasText: title });
    await expect(popover).toContainText("外部から取得した説明");
    await expect(popover.getByRole("button", { name: "編集" })).toHaveCount(0);
    await page.keyboard.press("Escape");

    await page.getByRole("button", { name: "サーバー設定" }).click();
    const refreshed = page.waitForResponse((response) =>
      response.url().includes(`${base}/${first.id}/refresh`),
    );
    await section.getByRole("button", { name: "今すぐ取得" }).click();
    expect((await (await refreshed).json()).last_error).toBeNull();
    await expect.poll(() => requests.length).toBe(2);
    expect(requests[1]).toBe('"sample-1"');
    await expect(
      section.getByRole("listitem").filter({ hasText: "大会日程" }),
    ).not.toContainText("取得できません");
    await page.keyboard.press("Escape");
    await expect(eventOn(page, title)).toBeVisible();
    await page.getByRole("button", { name: "サーバー設定" }).click();

    await section.getByLabel("表示名").fill("取得失敗");
    await section
      .getByLabel("ICS URL")
      .fill(`http://webhook.test:${address.port}/failure.ics`);
    const addedFailure = page.waitForResponse(
      (response) =>
        response.url().includes(base) && response.request().method() === "POST",
    );
    await section.getByRole("button", { name: "追加" }).click();
    expect((await addedFailure).status()).toBe(201);
    const second = (await (await page.request.get(base)).json()).find(
      (item: { name: string }) => item.name === "取得失敗",
    );
    created.push(second.id);
    await page.keyboard.press("Escape");
    await expect(
      page
        .getByRole("list", { name: "外部カレンダーの凡例" })
        .getByText("取得できません"),
    ).toBeVisible();
    await expect(eventOn(page, title)).toBeVisible();
    await expect.poll(() => failedRequests).toBe(1);
    const range = new URLSearchParams({
      start: "2026-10-01T00:00:00",
      end: "2026-10-02T00:00:00",
    });
    await page.request.get(`/local/api/events/${guildId}/external?${range}`);
    expect(failedRequests).toBe(1);
    const member = await page.request.get(
      `/local/api/guilds/${E2E_GUILDS.member.id}/external-calendars`,
    );
    expect(member.status()).toBe(200);
    const denied = await page.request.post(
      `/local/api/guilds/${E2E_GUILDS.member.id}/external-calendars`,
      {
        data: {
          url: "https://example.com/calendar.ics",
          name: "不可",
          color: "#2196F3",
        },
      },
    );
    expect(denied.status()).toBe(403);

    const redirected = await page.request.post(base, {
      data: {
        url: `http://webhook.test:${address.port}/private-redirect.ics`,
        name: "非公開への転送",
        color: "#2196F3",
      },
    });
    expect(redirected.status()).toBe(201);
    created.push((await redirected.json()).id);
    const checked = await page.request.get(
      `/local/api/events/${guildId}/external?${range}`,
    );
    const blocked = (await checked.json()).find(
      (item: { calendar: { name: string } }) =>
        item.calendar.name === "非公開への転送",
    );
    expect(blocked.calendar.last_error).toContain("公開インターネット");
    expect(blocked.events).toEqual([]);

    const exceptional = await page.request.post(base, {
      data: {
        url: `http://webhook.test:${address.port}/exception.ics`,
        name: "例外付き",
        color: "#2196F3",
      },
    });
    expect(exceptional.status()).toBe(201);
    created.push((await exceptional.json()).id);
    const withException = await page.request.get(
      `/local/api/events/${guildId}/external?${range}`,
    );
    const unsupported = (await withException.json()).find(
      (item: { calendar: { name: string } }) =>
        item.calendar.name === "例外付き",
    );
    expect(unsupported.calendar.last_error).toContain("例外付き");
    expect(unsupported.events).toEqual([]);
  } finally {
    for (const id of created) await page.request.delete(`${base}/${id}`);
    await new Promise<void>((resolve) => receiver.close(() => resolve()));
  }
});

test.describe("390px の外部カレンダー", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true });

  test("取得失敗が読め、設定画面が横にはみ出さない", async ({ page }) => {
    const guildId = E2E_GUILDS.admin.id;
    const base = `/local/api/guilds/${guildId}/external-calendars`;
    const created = await page.request.post(base, {
      data: {
        url: "http://127.0.0.1:1/calendar.ics",
        name: "確認用",
        color: "#2196F3",
      },
    });
    expect(created.status()).toBe(201);
    const id = (await created.json()).id;
    try {
      await page.goto(`/dashboard/${guildId}`);
      await expect(
        page
          .getByRole("list", { name: "外部カレンダーの凡例" })
          .getByText("取得できません"),
      ).toBeVisible();
      await page.getByRole("button", { name: "サーバー設定" }).click();
      const dialog = page.getByRole("dialog", { name: "サーバー設定" });
      await expect(dialog.getByLabel("ICS URL")).toBeVisible();
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
    } finally {
      await page.request.delete(`${base}/${id}`);
    }
  });
});
