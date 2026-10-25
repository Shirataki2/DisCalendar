import { expect, test } from "@playwright/test";
import { E2E_ACTIVITY_CODE, E2E_GUILDS } from "./fixtures";

// Discord の Activity (#350)。SDK は Discord の iframe の外では動かないので、
// 画面は入口の振り分けまで、認証はコード交換 → Bearer で api を呼べるところまでを確かめる

// cookie を持たない状態 (Activity の iframe と同じ) にする
test.use({ storageState: { cookies: [], origins: [] } });

test.describe("Activity のログイン", () => {
  test("コードを交換したトークンで api を呼べる (cookie なし)", async ({
    request,
  }) => {
    const guildPath = `/local/api/guilds/${E2E_GUILDS.admin.id}`;
    // トークンが無ければ未ログイン扱い
    expect((await request.get(guildPath)).status()).toBe(401);

    const signIn = await request.post("/api/auth/discord-activity/sign-in", {
      data: { code: E2E_ACTIVITY_CODE },
    });
    expect(signIn.status()).toBe(200);
    // cookie は返さない (iframe では届かないので、トークンだけで認証する)
    expect(signIn.headers()["set-cookie"]).toBeUndefined();
    const { token } = (await signIn.json()) as { token: string };
    expect(token).toContain(".");

    const guild = await request.get(guildPath, {
      headers: { authorization: `Bearer ${token}` },
    });
    expect(guild.status()).toBe(200);
    expect((await guild.json()).name).toBe(E2E_GUILDS.admin.name);

    // 権限は通常の Web と同じく api が判定する (Bot のいないサーバーは 403)。
    // api はギルドの取得結果をキャッシュするので、招待のテスト (invite.spec.ts) が Bot を参加させる
    // E2E_GUILDS.invitable ではなく、どのテストも使わない ID で確かめる
    const notJoined = await request.get(
      "/local/api/guilds/200000000000000099",
      { headers: { authorization: `Bearer ${token}` } },
    );
    expect(notJoined.status()).toBe(403);
  });

  test("不正なコードは 401 になる", async ({ request }) => {
    const signIn = await request.post("/api/auth/discord-activity/sign-in", {
      data: { code: "invalid-code" },
    });
    expect(signIn.status()).toBe(401);
  });

  test("改ざんしたトークンは受け付けない", async ({ request }) => {
    const res = await request.get(`/local/api/guilds/${E2E_GUILDS.admin.id}`, {
      headers: { authorization: "Bearer forged.c2lnbmF0dXJl" },
    });
    expect(res.status()).toBe(401);
  });
});

test.describe("Activity の入口", () => {
  test("Discord が開くルート (frame_id 付き) は Activity の画面になる", async ({
    page,
  }) => {
    await page.goto("/?frame_id=e2e&instance_id=e2e&platform=desktop");
    // SDK は Discord の応答を待ち続けるので読み込み中のまま。LP は出ない
    await expect(page.getByText("読み込み中…")).toBeVisible();
    await expect(page).toHaveURL(/\/\?frame_id=e2e/);
    await expect(page.getByRole("link", { name: "ログイン" })).toHaveCount(0);
  });

  test("ブラウザで直接開くと Activity 専用の案内を出す", async ({ page }) => {
    await page.goto("/activity");
    await expect(
      page.getByRole("heading", {
        name: "この画面は Discord のアクティビティ専用です",
      }),
    ).toBeVisible();
  });

  test("画面の枠は表示領域の高さに固定される", async ({ page }) => {
    // カレンダーは残りの高さ (flex-1 + min-h-0) を使うので、枠の高さが決まっていないと本体が 0 px になる。
    // SDK は Discord の外では動かずカレンダーまで進めないので、カレンダーを入れる枠 (main の親) の高さで確かめる
    // 枠の高さが min-height だけだと、中の % 指定の高さ (FullCalendar が使う) が 0 になる。
    // 枠に height: 100% の要素を入れ、表示領域の高さになることを見る
    await page.goto("/activity");
    const frame = page.locator("main").locator("..");
    await expect(frame).toBeVisible();
    const probeHeight = await frame.evaluate((element) => {
      const probe = document.createElement("div");
      probe.style.height = "100%";
      probe.style.flexShrink = "0";
      element.append(probe);
      const height = probe.getBoundingClientRect().height;
      probe.remove();
      return height;
    });
    expect(probeHeight).toBe(page.viewportSize()?.height);
  });

  test("クエリの無いルートは従来どおり LP を出す", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByText("読み込み中…")).toHaveCount(0);
  });
});
