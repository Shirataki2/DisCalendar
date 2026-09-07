import { expect, test } from "@playwright/test";
import { E2E_GUILDS } from "./fixtures";

// 通知 API が無効な headless shell を避け、通常の Chromium の headless を使う。
test.use({ channel: "chromium" });

// プッシュサービスだけを代替し、通知の許可・Service Worker・購読 API は実物を使う。
test("通知範囲と端末の登録・再読込・解除", async ({ page, context }) => {
  await context.grantPermissions(["notifications"]);
  await page.addInitScript(() => {
    let current: PushSubscription | null = null;
    PushManager.prototype.getSubscription = async () => current;
    PushManager.prototype.subscribe = async () => {
      const keys = await crypto.subtle.generateKey(
        { name: "ECDH", namedCurve: "P-256" },
        true,
        ["deriveBits"],
      );
      const publicKey = new Uint8Array(
        await crypto.subtle.exportKey("raw", keys.publicKey),
      );
      const encode = (bytes: Uint8Array) =>
        btoa(String.fromCharCode(...bytes))
          .replace(/\+/g, "-")
          .replace(/\//g, "_")
          .replace(/=+$/, "");
      const endpoint = "https://fcm.googleapis.com/fcm/send/e2e-device";
      current = {
        endpoint,
        options: {},
        toJSON: () => ({
          endpoint,
          keys: {
            p256dh: encode(publicKey),
            auth: encode(crypto.getRandomValues(new Uint8Array(16))),
          },
        }),
        unsubscribe: async () => {
          current = null;
          return true;
        },
      } as unknown as PushSubscription;
      return current;
    };
  });
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(page.getByRole("grid")).toBeVisible();
  await context.grantPermissions(["notifications"], {
    origin: new URL(page.url()).origin,
  });
  expect(await page.evaluate(() => Notification.permission)).toBe("granted");
  // dev では自動登録しないため、テストからだけ実際の SW を登録する。
  await page.evaluate(async () => {
    await navigator.serviceWorker.register("/serwist/sw.js", { scope: "/" });
    await navigator.serviceWorker.ready;
  });
  const open = async () => {
    await page.getByRole("button", { name: "アカウントメニュー" }).click();
    await page.getByRole("menuitem", { name: "プッシュ通知" }).click();
  };
  await open();
  const dialog = page.getByRole("dialog", { name: "プッシュ通知" });
  await dialog.getByRole("radio", { name: "自分が作った予定だけ" }).click();
  await expect(
    dialog.getByRole("radio", { name: "自分が作った予定だけ" }),
  ).toBeChecked();
  await dialog.getByLabel("端末名").fill("E2E の端末");
  await dialog.getByRole("button", { name: "この端末で受け取る" }).click();
  await expect(
    dialog.getByRole("button", { name: "E2E の端末の登録を解除" }),
  ).toBeVisible();
  await page.screenshot({
    path: "/tmp/issue176-push-registered.png",
    animations: "disabled",
  });
  await page.reload();
  await open();
  await expect(
    dialog.getByRole("radio", { name: "自分が作った予定だけ" }),
  ).toBeChecked();
  await dialog.getByRole("button", { name: "E2E の端末の登録を解除" }).click();
  await expect(dialog.getByText("登録された端末はありません。")).toBeVisible();
  await dialog.getByRole("radio", { name: "オフ", exact: true }).click();
  await expect(
    dialog.getByRole("radio", { name: "オフ", exact: true }),
  ).toBeChecked();
  await page.screenshot({
    path: "/tmp/issue176-push-settings.png",
    animations: "disabled",
  });
});

test("ドロワーから通知設定を開けて、通知リンクの該当日を表示する", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}?date=2027-02-03`);
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(page.getByText("2027年2月", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "メニュー", exact: true }).click();
  await page.getByRole("button", { name: "プッシュ通知", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "プッシュ通知" }),
  ).toBeVisible();
  await expect(page.getByText(/iPhone \/ iPad では/)).toBeVisible();
  await page.screenshot({
    path: "/tmp/issue176-push-mobile.png",
    animations: "disabled",
  });
});

test("購読 API は未ログインのアクセスを拒否する", async ({
  playwright,
  baseURL,
}) => {
  const client = await playwright.request.newContext({
    baseURL,
    storageState: { cookies: [], origins: [] },
  });
  try {
    for (const [method, url, data] of [
      ["GET", "/local/api/users/@me/push-subscriptions", undefined],
      [
        "POST",
        "/local/api/users/@me/push-subscriptions",
        {
          endpoint: "https://fcm.googleapis.com/test",
          p256dh: "test",
          auth: "test",
          device_name: "端末",
        },
      ],
      ["PUT", "/local/api/users/@me/push-settings", { scope: "all" }],
      ["DELETE", "/local/api/users/@me/push-subscriptions/1", undefined],
      [
        "DELETE",
        "/local/api/users/@me/push-subscriptions",
        { endpoint: "https://fcm.googleapis.com/test" },
      ],
    ] as const) {
      const response = await client.fetch(url, { method, data });
      expect(response.status()).toBe(401);
    }
  } finally {
    await client.dispose();
  }
});
