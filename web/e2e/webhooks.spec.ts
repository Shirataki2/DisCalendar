import { createHmac } from "node:crypto";
import { createServer } from "node:http";
import { expect, test } from "@playwright/test";
import { E2E_GUILDS, E2E_USER } from "./fixtures";

// webhook-e2e 機能付き API だけが webhook.test をループバックへ解決する。
// 本番ビルドの SSRF 判定には例外を入れない。
test("Webhook の登録・署名付き配信・ログ・変更と削除・権限境界", async ({
  page,
}) => {
  const received: { body: string; signature: string | undefined }[] = [];
  const receiver = createServer(async (req, res) => {
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    received.push({
      body: Buffer.concat(chunks).toString(),
      signature: req.headers["x-discalendar-signature"] as string | undefined,
    });
    res.writeHead(204).end();
  });
  await new Promise<void>((resolve) =>
    receiver.listen(0, "127.0.0.1", resolve),
  );
  const address = receiver.address();
  if (!address || typeof address === "string")
    throw new Error("受信モックの起動に失敗");
  const guild = E2E_GUILDS.admin.id;
  const base = `/local/api/guilds/${guild}/webhooks`;
  let hookId: string | undefined;
  let eventId: number | undefined;
  try {
    await page.goto(`/dashboard/${guild}`);
    await page.getByRole("button", { name: "サーバー設定" }).click();
    const dialog = page.getByRole("dialog", { name: "サーバー設定" });
    await dialog
      .getByLabel("Webhook URL", { exact: true })
      .fill(`http://webhook.test:${address.port}/secret-path`);
    await dialog.getByRole("button", { name: /Webhook を登録/ }).click();
    const secret = await dialog
      .getByLabel("署名用シークレット（一度だけ表示）")
      .inputValue();
    expect(secret).toMatch(/^[0-9a-f]{64}$/);
    const hooksResponse = await page.request.get(base);
    const hooks = await hooksResponse.json();
    const hook = hooks.find((h: { url: string }) =>
      h.url.includes(String(address.port)),
    );
    hookId = hook.id;
    expect(JSON.stringify(hooks)).not.toContain(secret);
    expect(JSON.stringify(hooks)).not.toContain("secret-path");
    const input = {
      name: `Webhook ${Date.now()}`,
      description: "説明も通知",
      color: "#5865F2",
      notifications: [],
      is_all_day: false,
      start_at: "2026-10-01T10:00:00",
      end_at: "2026-10-01T11:00:00",
    };
    const created = await page.request.post(`/local/api/events/${guild}`, {
      data: input,
    });
    expect(created.status()).toBe(201);
    const event = await created.json();
    eventId = event.id;
    await expect.poll(() => received.length).toBe(1);
    const payload = JSON.parse(received[0].body);
    expect(payload.type).toBe("event.created");
    expect(payload.event).toEqual(event);
    expect(payload.actor_id).toBe(E2E_USER.discordId);
    expect(received[0].signature).toBe(
      `sha256=${createHmac("sha256", secret).update(received[0].body).digest("hex")}`,
    );
    await dialog
      .getByText("直近の配信ログ（最大20件）", { exact: true })
      .click();
    await expect(dialog.getByText(/event.created · 204/)).toBeVisible({
      timeout: 15000,
    });
    expect(
      (
        await page.request.put(`/local/api/events/${guild}/${eventId}`, {
          data: { ...input, name: "Webhook 更新" },
        })
      ).ok(),
    ).toBeTruthy();
    expect(
      (
        await page.request.delete(`/local/api/events/${guild}/${eventId}`)
      ).status(),
    ).toBe(204);
    eventId = undefined;
    await expect.poll(() => received.length).toBe(3);
    expect(received.slice(1).map((r) => JSON.parse(r.body).type)).toEqual([
      "event.updated",
      "event.deleted",
    ]);
    expect(JSON.parse(received[2].body).event.name).toBe("Webhook 更新");
    const denied = await page.request.get(
      `/local/api/guilds/${E2E_GUILDS.member.id}/webhooks`,
    );
    expect(denied.status()).toBe(403);
    expect(
      (
        await page.request.put(
          `/local/api/guilds/${E2E_GUILDS.member.id}/webhooks/${hookId}`,
          { data: { enabled: false } },
        )
      ).status(),
    ).toBe(403);
    const unsafe = await page.request.post(base, {
      data: { url: "http://127.0.0.1:80/private", kind: "json" },
    });
    expect(unsafe.status()).toBe(400);
    await page.request.put(`${base}/${hookId}`, { data: { enabled: false } });
    expect((await page.request.post(`${base}/${hookId}/test`)).status()).toBe(
      400,
    );
  } finally {
    if (eventId)
      await page.request.delete(`/local/api/events/${guild}/${eventId}`);
    if (hookId) await page.request.delete(`${base}/${hookId}`);
    await new Promise<void>((resolve) => receiver.close(() => resolve()));
  }
});
