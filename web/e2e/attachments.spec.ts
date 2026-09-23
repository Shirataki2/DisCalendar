import { expect, test } from "@playwright/test";
import { Pool } from "pg";
import { ATTACHMENT_MOCK_URL } from "./attachment-storage";
import { openEventPopover } from "./calendar";
import { setEditorRoles } from "./discord-mock";
import { DATABASE_URL, WEB_URL } from "./env";
import { E2E_EDITOR_ROLES, E2E_GUILDS } from "./fixtures";

const guild = E2E_GUILDS.admin.id;
const png = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a3l8AAAAASUVORK5CYII=",
  "base64",
);
const pdf = Buffer.from("%PDF-1.7\n添付テスト\n%%EOF");
const eventInput = {
  name: "添付API",
  color: "#123456",
  start_at: "2026-09-17T10:00:00",
  end_at: "2026-09-17T11:00:00",
  notifications: [],
};

test("新規予定の部分失敗から添付だけ再試行し、画像確認・ダウンロード・削除できる", async ({
  page,
}) => {
  const title = `添付 ${Date.now()}`;
  await page.goto(`/dashboard/${guild}`);
  await page.getByRole("button", { name: "新規作成" }).click();
  await page.getByRole("menuitem", { name: /予定を作成/ }).click();
  const form = page.getByRole("dialog", { name: "予定を作成" });
  await form.getByLabel("タイトル").fill(` ${title} `);
  await expect(
    form.getByLabel("ファイルを添付", { exact: true }),
  ).toBeEnabled();
  await form.getByLabel("ファイルを添付", { exact: true }).setInputFiles([
    { name: "案内.png", mimeType: "image/png", buffer: png },
    { name: "資料.pdf", mimeType: "application/pdf", buffer: pdf },
  ]);
  let fail = true;
  await page.route(`${ATTACHMENT_MOCK_URL}/**`, async (route) => {
    if (
      fail &&
      route.request().method() === "PUT" &&
      route.request().headers()["content-type"] === "application/pdf"
    ) {
      await route.abort();
      return;
    }
    await route.continue();
  });
  const created = page.waitForResponse(
    (r) =>
      r.url().endsWith(`/local/api/events/${guild}`) &&
      r.request().method() === "POST",
  );
  await form.getByRole("button", { name: "作成", exact: true }).click();
  const event = await (await created).json();
  const edit = page.getByRole("dialog", { name: "予定を編集" });
  await expect(
    edit.getByText("予定は保存済みです。", { exact: false }),
  ).toBeVisible();
  await expect(
    edit.getByRole("button", { name: "添付を再試行" }),
  ).toBeEnabled();
  expect(
    await (
      await page.request.get(
        `/local/api/events/${guild}/${event.id}/attachments`,
      )
    ).json(),
  ).toHaveLength(1);
  fail = false;
  await edit.getByRole("button", { name: "添付を再試行" }).click();
  await expect(edit.getByRole("button", { name: "ダウンロード" })).toHaveCount(
    2,
  );
  await edit.getByRole("button", { name: "キャンセル" }).click();
  const popover = await openEventPopover(page, title);
  await expect(popover.getByRole("img", { name: "案内.png" })).toBeVisible();
  await expect
    .poll(() =>
      popover
        .getByRole("img", { name: "案内.png" })
        .evaluate((img: HTMLImageElement) => img.naturalWidth),
    )
    .toBeGreaterThan(0);
  const download = page.waitForEvent("download");
  await popover.getByRole("button", { name: "ダウンロード" }).last().click();
  expect((await download).suggestedFilename()).toBe("資料.pdf");
  await popover.getByRole("button", { name: "編集", exact: true }).click();
  page.once("dialog", (dialog) => dialog.accept());
  await edit.getByRole("button", { name: "添付を削除" }).first().click();
  await expect(edit.getByRole("button", { name: "ダウンロード" })).toHaveCount(
    1,
  );
  const pool = new Pool({ connectionString: DATABASE_URL });
  try {
    expect(
      (
        await pool.query(
          "SELECT count(*)::int AS count FROM events WHERE name=$1",
          [title],
        )
      ).rows[0].count,
    ).toBe(1);
  } finally {
    await pool.end();
    await page.request.delete(`/local/api/events/${guild}/${event.id}`);
  }
});

test("APIは認証・ギルド境界・サイズ・形式・署名と確定の冪等性を守る", async ({
  page,
  playwright,
}) => {
  const anonymous = await playwright.request.newContext({
    baseURL: WEB_URL,
    storageState: { cookies: [], origins: [] },
  });
  const created = await page.request.post(`/local/api/events/${guild}`, {
    data: eventInput,
  });
  const event = await created.json();
  const base = `/local/api/events/${guild}/${event.id}/attachments`;
  const data = {
    filename: "案内.png",
    content_type: "image/png",
    size: png.length,
  };
  try {
    expect((await anonymous.get(base)).status()).toBe(401);
    expect((await anonymous.post(base, { data })).status()).toBe(401);
    expect(
      (
        await page.request.get(
          `/local/api/events/${E2E_GUILDS.member.id}/${event.id}/attachments`,
        )
      ).status(),
    ).toBe(404);
    for (const invalid of [
      { ...data, size: 0 },
      { ...data, size: 10485761 },
      { ...data, filename: "案内.svg" },
    ]) {
      expect((await page.request.post(base, { data: invalid })).status()).toBe(
        400,
      );
    }
    const reserved = await (await page.request.post(base, { data })).json();
    expect(await (await page.request.get(base)).json()).toHaveLength(0);
    expect(
      (await page.request.post(`${base}/${reserved.id}/url`)).status(),
    ).toBe(404);
    expect(
      (
        await page.request.put(reserved.upload.url, {
          headers: reserved.upload.headers,
          data: Buffer.alloc(1),
        })
      ).status(),
    ).toBe(403);
    expect(
      (
        await page.request.put(reserved.upload.url, {
          headers: { ...reserved.upload.headers, "content-type": "text/html" },
          data: png,
        })
      ).status(),
    ).toBe(403);
    expect(
      (
        await page.request.put(reserved.upload.url, {
          headers: reserved.upload.headers,
          data: png,
        })
      ).status(),
    ).toBe(200);
    const complete = `${base}/${reserved.id}/complete`;
    expect((await page.request.post(complete)).status()).toBe(200);
    expect((await page.request.post(complete)).status()).toBe(200);
    expect(await (await page.request.get(base)).json()).toHaveLength(1);
    expect(
      (
        await page.request.put(reserved.upload.url, {
          headers: reserved.upload.headers,
          data: png,
        })
      ).status(),
    ).toBe(412);
    expect((await anonymous.post(`${base}/${reserved.id}/url`)).status()).toBe(
      401,
    );
    const fake = await (await page.request.post(base, { data })).json();
    await page.request.put(fake.upload.url, {
      headers: fake.upload.headers,
      data: Buffer.alloc(png.length, 65),
    });
    expect(
      (await page.request.post(`${base}/${fake.id}/complete`)).status(),
    ).toBe(400);
    expect(await (await page.request.get(base)).json()).toHaveLength(1);
  } finally {
    await anonymous.dispose();
    await page.request.delete(`/local/api/events/${guild}/${event.id}`);
  }
});

test("閲覧専用でも添付を確認でき、編集ロールなしの変更を拒否する", async ({
  page,
}) => {
  const member = E2E_GUILDS.member.id;
  const pool = new Pool({ connectionString: DATABASE_URL });
  const previous = (
    await pool.query(
      "SELECT restricted,editor_role_ids FROM guild_config WHERE guild_id=$1",
      [member],
    )
  ).rows[0];
  await pool.query(
    "INSERT INTO guild_config(guild_id,restricted) VALUES ($1,false) ON CONFLICT(guild_id) DO UPDATE SET restricted=false",
    [member],
  );
  const created = await page.request.post(`/local/api/events/${member}`, {
    data: eventInput,
  });
  const event = await created.json();
  const base = `/local/api/events/${member}/${event.id}/attachments`;
  try {
    const data = {
      filename: "資料.pdf",
      content_type: "application/pdf",
      size: pdf.length,
    };
    const reserved = await (await page.request.post(base, { data })).json();
    await page.request.put(reserved.upload.url, {
      headers: reserved.upload.headers,
      data: pdf,
    });
    expect(
      (await page.request.post(`${base}/${reserved.id}/complete`)).status(),
    ).toBe(200);
    await pool.query(
      "INSERT INTO guild_config(guild_id,restricted) VALUES ($1,true) ON CONFLICT(guild_id) DO UPDATE SET restricted=true,editor_role_ids='{}'",
      [member],
    );
    expect((await page.request.get(base)).status()).toBe(200);
    expect(
      (await page.request.post(`${base}/${reserved.id}/url`)).status(),
    ).toBe(200);
    expect((await page.request.post(base, { data })).status()).toBe(403);
    expect(
      (await page.request.post(`${base}/${reserved.id}/complete`)).status(),
    ).toBe(403);
    expect((await page.request.delete(`${base}/${reserved.id}`)).status()).toBe(
      403,
    );
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(`/dashboard/${member}?date=2026-09-17`);
    const popover = await openEventPopover(page, eventInput.name);
    await expect(
      popover.getByRole("button", { name: "ダウンロード" }),
    ).toBeVisible();
    await expect(
      popover.getByRole("button", { name: "添付を削除" }),
    ).toHaveCount(0);
    await pool.query(
      "UPDATE guild_config SET editor_role_ids=$2 WHERE guild_id=$1",
      [member, [E2E_EDITOR_ROLES[0].id]],
    );
    await setEditorRoles(member, [E2E_EDITOR_ROLES[0].id]);
    expect(
      (
        await page.request.post(
          `/local/api/guilds/${member}/@me/permissions/refresh`,
        )
      ).status(),
    ).toBe(200);
    expect((await page.request.delete(`${base}/${reserved.id}`)).status()).toBe(
      204,
    );
  } finally {
    await pool.query(
      "UPDATE guild_config SET restricted=false WHERE guild_id=$1",
      [member],
    );
    await page.request.delete(`/local/api/events/${member}/${event.id}`);
    await setEditorRoles(member, []);
    if (previous)
      await pool.query(
        "UPDATE guild_config SET restricted=$2,editor_role_ids=$3 WHERE guild_id=$1",
        [member, previous.restricted, previous.editor_role_ids],
      );
    else
      await pool.query("DELETE FROM guild_config WHERE guild_id=$1", [member]);
    await pool.end();
  }
});

test("確定時のR2障害・差し替えと予定削除との競合でも未検証ファイルを公開しない", async ({
  page,
}) => {
  const pool = new Pool({ connectionString: DATABASE_URL });
  const event = await (
    await page.request.post(`/local/api/events/${guild}`, { data: eventInput })
  ).json();
  const base = `/local/api/events/${guild}/${event.id}/attachments`;
  const data = {
    filename: "案内.png",
    content_type: "image/png",
    size: png.length,
  };
  try {
    const first = await (await page.request.post(base, { data })).json();
    await page.request.put(first.upload.url, {
      headers: first.upload.headers,
      data: png,
    });
    await page.request.get(`${ATTACHMENT_MOCK_URL}/test-control?replace=true`);
    expect(
      (await page.request.post(`${base}/${first.id}/complete`)).status(),
    ).toBe(503);
    expect(await (await page.request.get(base)).json()).toHaveLength(0);
    await page.request.get(`${ATTACHMENT_MOCK_URL}/test-control?copy=fail`);
    expect(
      (await page.request.post(`${base}/${first.id}/complete`)).status(),
    ).toBe(503);
    expect(await (await page.request.get(base)).json()).toHaveLength(0);
    await page.request.get(`${ATTACHMENT_MOCK_URL}/test-control?hold=true`);
    const finishing = page.request.post(`${base}/${first.id}/complete`);
    await expect
      .poll(
        async () =>
          (
            await (
              await page.request.get(
                `${ATTACHMENT_MOCK_URL}/test-control?status=1`,
              )
            ).json()
          ).copyStarted,
      )
      .toBe(true);
    const deleting = page.request.delete(
      `/local/api/events/${guild}/${event.id}`,
    );
    await page.request.get(`${ATTACHMENT_MOCK_URL}/test-control`);
    expect((await finishing).status()).toBe(200);
    expect((await deleting).status()).toBe(204);
    expect((await page.request.get(base)).status()).toBe(404);
    const queued = await pool.query(
      "SELECT object_key FROM attachment_deletions WHERE object_key IN ($1,$2)",
      [`temporary/${first.id}`, `attachments/${first.id}`],
    );
    expect(queued.rowCount).toBe(2);
  } finally {
    await page.request.get(`${ATTACHMENT_MOCK_URL}/test-control`);
    await page.request.delete(`/local/api/events/${guild}/${event.id}`);
    await pool.end();
  }
});

test("既存予定への送信を中断すると予定は残り、未確定の添付を回収する", async ({
  page,
}) => {
  const title = `添付中断 ${Date.now()}`;
  const event = await (
    await page.request.post(`/local/api/events/${guild}`, {
      data: { ...eventInput, name: title },
    })
  ).json();
  const pool = new Pool({ connectionString: DATABASE_URL });
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let uploading = false;
  await page.route(`${ATTACHMENT_MOCK_URL}/**`, async (route) => {
    if (route.request().method() === "PUT") {
      uploading = true;
      await held;
      await route.abort().catch(() => {});
    } else await route.continue();
  });
  try {
    await page.goto(`/dashboard/${guild}?date=2026-09-17`);
    const popover = await openEventPopover(page, title);
    await popover.getByRole("button", { name: "編集", exact: true }).click();
    const edit = page.getByRole("dialog", { name: "予定を編集" });
    await expect(
      edit.getByLabel("ファイルを添付", { exact: true }),
    ).toBeEnabled();
    await edit.getByLabel("ファイルを添付", { exact: true }).setInputFiles({
      name: "中断.png",
      mimeType: "image/png",
      buffer: png,
    });
    await edit.getByRole("button", { name: "保存", exact: true }).click();
    await expect.poll(() => uploading).toBe(true);
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "破棄して閉じる" }).click();
    await expect(edit).not.toBeVisible();
    release();
    await expect
      .poll(
        async () =>
          (
            await pool.query(
              "SELECT id FROM event_attachments WHERE event_id=$1",
              [event.id],
            )
          ).rowCount,
      )
      .toBe(0);
    expect(
      (
        await page.request.get(
          `/local/api/events/${guild}/${event.id}/attachments`,
        )
      ).status(),
    ).toBe(200);
    await expect(page.getByRole("dialog")).toHaveCount(0);
  } finally {
    release();
    await page.request.delete(`/local/api/events/${guild}/${event.id}`);
    await pool.end();
  }
});

test("保存先が未設定でも添付以外の予定作成を続けられる", async ({ page }) => {
  await page.route(
    `**/local/api/guilds/${guild}/attachments`,
    async (route) => {
      const response = await route.fetch();
      await route.fulfill({
        response,
        json: { ...(await response.json()), enabled: false },
      });
    },
  );
  await page.goto(`/dashboard/${guild}`);
  await page.getByRole("button", { name: "新規作成" }).click();
  await page.getByRole("menuitem", { name: /予定を作成/ }).click();
  const form = page.getByRole("dialog", { name: "予定を作成" });
  await expect(
    form.getByText("添付ファイルは現在利用できません"),
  ).toBeVisible();
  await expect(
    form.getByLabel("ファイルを添付", { exact: true }),
  ).toBeDisabled();
  await form.getByLabel("タイトル").fill("添付なしの予定");
  const created = page.waitForResponse(
    (r) =>
      r.url().endsWith(`/local/api/events/${guild}`) &&
      r.request().method() === "POST",
  );
  await form.getByRole("button", { name: "作成", exact: true }).click();
  const response = await created;
  expect(response.ok()).toBe(true);
  await expect(form).not.toBeVisible();
  await page.request.delete(
    `/local/api/events/${guild}/${(await response.json()).id}`,
  );
});

test("予定保存の応答前に閉じても、次に開いたフォームを上書きしない", async ({
  page,
}) => {
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let savedId: number | undefined;
  await page.route(`**/local/api/events/${guild}`, async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    const response = await route.fetch();
    savedId = (await response.json()).id;
    await held;
    await route.fulfill({ response });
  });
  try {
    await page.goto(`/dashboard/${guild}`);
    await page.getByRole("button", { name: "新規作成" }).click();
    await page.getByRole("menuitem", { name: /予定を作成/ }).click();
    const form = page.getByRole("dialog", { name: "予定を作成" });
    await form.getByLabel("タイトル").fill("保存応答待ちの予定");
    await expect(
      form.getByLabel("ファイルを添付", { exact: true }),
    ).toBeEnabled();
    await form
      .getByLabel("ファイルを添付", { exact: true })
      .setInputFiles({ name: "案内.png", mimeType: "image/png", buffer: png });
    await form.getByRole("button", { name: "作成", exact: true }).click();
    await expect.poll(() => savedId).toBeDefined();
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "破棄して閉じる" }).click();
    await expect(form).not.toBeVisible();
    await page.getByRole("button", { name: "新規作成" }).click();
    await page.getByRole("menuitem", { name: /予定を作成/ }).click();
    await form.getByLabel("タイトル").fill("次の予定");
    const response = page.waitForResponse(
      (r) =>
        r.url().endsWith(`/local/api/events/${guild}`) &&
        r.request().method() === "POST",
    );
    release();
    await response;
    await expect(form.getByLabel("タイトル")).toHaveValue("次の予定");
    await expect(
      form.getByLabel("ファイルを添付", { exact: true }),
    ).toBeEnabled();
    expect(
      await (
        await page.request.get(
          `/local/api/events/${guild}/${savedId}/attachments`,
        )
      ).json(),
    ).toHaveLength(0);
  } finally {
    release();
    if (savedId)
      await page.request.delete(`/local/api/events/${guild}/${savedId}`);
  }
});
