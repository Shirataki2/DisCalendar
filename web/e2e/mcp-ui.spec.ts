import { expect, test } from "@playwright/test";
import { Pool } from "pg";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS, E2E_USER } from "./fixtures";

for (const width of [390, 1280]) {
  test(`MCPログインと接続管理のナビゲーション (${width}px)`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 844 });
    await page.goto("/mcp/connections");
    await expect(
      page.getByRole("heading", { name: "MCP 接続管理" }),
    ).toBeVisible();
    if (width < 1024)
      await page.getByRole("button", { name: "メニュー", exact: true }).click();
    const nav = page.getByRole("navigation", { name: "サイト内メニュー" });
    await expect(
      nav.getByRole("link", { name: "MCP 接続管理" }),
    ).toHaveAttribute("aria-current", "page");
    await nav.getByRole("link", { name: "すべての予定" }).click();
    await expect(page).toHaveURL("/dashboard/all");
    await page.context().clearCookies();
    await page.goto("/mcp/login?state=keep-me");
    await expect(page.getByRole("banner")).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Discordでログイン" }),
    ).toBeVisible();
    await page.screenshot({
      path: test.info().outputPath(`login-${width}.png`),
      fullPage: true,
      caret: "initial",
    });
    let finish: () => void = () => {};
    const pending = new Promise<void>((resolve) => {
      finish = resolve;
    });
    await page.route("**/mcp/login/start", async (route) => {
      expect(route.request().postDataJSON()).toEqual({
        oauth_query: "state=keep-me",
      });
      await pending;
      await route.fulfill({ status: 400, json: {} });
    });
    await page.getByRole("button", { name: "Discordでログイン" }).click();
    await expect(
      page.getByRole("button", { name: "ログイン画面へ移動中…" }),
    ).toBeDisabled();
    await expect(
      page.getByText(
        "Discordでログインした後、接続先のクライアントと許可するサーバー・操作を確認します。",
        { exact: true },
      ),
    ).toBeVisible();
    finish();
    await expect(page.locator("main").getByRole("alert")).toContainText(
      "ログインを開始できませんでした",
    );
  });

  test(`MCP接続一覧とキーボード解除 (${width}px)`, async ({ page }) => {
    test.skip(
      process.env.MCP_ENABLED !== "true",
      "MCP_ENABLED=true で専用DDLを準備して実行",
    );
    const pool = new Pool({ connectionString: DATABASE_URL });
    const id = "23000000-0000-4000-8000-000000000001";
    try {
      await pool.query(
        `INSERT INTO mcp_connections (id, user_id, discord_account_id, client_id, scopes, guild_ids, created_at)
        SELECT $1, $2, id, $3, $4, $5, '2026-09-15T00:00:00Z' FROM account WHERE "userId" = $2 AND "providerId" = 'discord' LIMIT 1`,
        [
          id,
          E2E_USER.id,
          "https://client.example/metadata.json",
          ["guilds:read", "offline_access"],
          [E2E_GUILDS.admin.id],
        ],
      );
      await page.setViewportSize({ width, height: 844 });
      await page.goto("/mcp/connections");
      const connection = page.getByRole("article");
      await expect(connection).toContainText(E2E_GUILDS.admin.name);
      await expect(connection).toContainText("2026/09/15 9:00 (日本時間)");
      await expect(connection).toContainText("未使用");
      await page.screenshot({
        path: test.info().outputPath(`connections-${width}.png`),
        fullPage: true,
        caret: "initial",
      });
      const revoke = connection.getByRole("button", { name: "接続を解除" });
      await revoke.focus();
      await page.keyboard.press("Enter");
      await expect(
        page.getByText("接続はありません。", { exact: false }),
      ).toBeVisible();
      expect(
        (
          await pool.query(
            "SELECT revoked_at FROM mcp_connections WHERE id = $1",
            [id],
          )
        ).rows[0].revoked_at,
      ).not.toBeNull();
      await page.screenshot({
        path: test.info().outputPath(`empty-${width}.png`),
        fullPage: true,
        caret: "initial",
      });
    } finally {
      await pool.query("DELETE FROM mcp_connections WHERE id = $1", [id]);
      await pool.end();
    }
  });

  test(`MCP同意の全選択・個別変更・送信失敗 (${width}px)`, async ({ page }) => {
    test.skip(
      process.env.MCP_ENABLED !== "true",
      "MCP_ENABLED=true で実行する同意画面の検証",
    );
    await page.setViewportSize({ width, height: 844 });
    await page.goto(
      "/mcp/consent?client_id=https%3A%2F%2Fclient.example%2Fmetadata.json&scope=guilds%3Aread+events%3Aread+offline_access",
    );
    const scopes = page.getByRole("group", {
      name: "許可する操作",
      exact: true,
    });
    const guilds = page.getByRole("group", {
      name: "許可するサーバー",
      exact: true,
    });
    await expect(scopes.getByRole("checkbox")).toHaveCount(3);
    for (const box of await page.getByRole("checkbox").all())
      await expect(box).toBeChecked();
    await expect(
      guilds.getByRole("checkbox", { name: E2E_GUILDS.invitable.name }),
    ).toHaveCount(0);
    await scopes.getByRole("button", { name: "すべて解除" }).click();
    await expect(
      page.getByRole("button", { name: "選択した内容を許可" }),
    ).toBeDisabled();
    await expect(
      page.getByRole("button", { name: "拒否", exact: true }),
    ).toBeEnabled();
    await scopes.getByRole("button", { name: "すべて選択" }).click();
    const offline = scopes.getByRole("checkbox", {
      name: "接続を継続して利用",
    });
    await offline.focus();
    await page.keyboard.press("Space");
    await expect(offline).not.toBeChecked();
    await guilds.getByRole("button", { name: "すべて解除" }).click();
    await expect(
      page.getByText("許可する操作とサーバーをそれぞれ1件以上"),
    ).toBeVisible();
    await guilds.getByRole("button", { name: "すべて選択" }).click();
    await guilds
      .getByRole("checkbox", { name: E2E_GUILDS.member.name, exact: true })
      .uncheck();
    await page.screenshot({
      path: test.info().outputPath(`consent-${width}.png`),
      fullPage: true,
      caret: "initial",
    });
    let finish: () => void = () => {};
    const pending = new Promise<void>((resolve) => {
      finish = resolve;
    });
    await page.route("**/mcp/consent/submit", async (route) => {
      const form = await new Response(route.request().postData(), {
        headers: { "Content-Type": route.request().headers()["content-type"] },
      }).formData();
      expect(form.get("accept")).toBe("true");
      expect(form.getAll("scope")).toEqual(["guilds:read", "events:read"]);
      expect(form.getAll("guild_id")).not.toContain(E2E_GUILDS.member.id);
      await pending;
      await route.fulfill({ status: 400, body: "失敗" });
    });
    await page.getByRole("button", { name: "選択した内容を許可" }).click();
    await expect(
      page.getByText("処理中です。このままお待ちください。"),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "拒否", exact: true }),
    ).toBeDisabled();
    finish();
    await expect(page.locator("main").getByRole("alert")).toContainText(
      "処理を完了できませんでした",
    );
    await expect(offline).not.toBeChecked();
    await expect(page.locator("main").getByRole("alert")).toBeInViewport();
    await expect(page.locator("main").getByRole("alert")).toBeFocused();
    await page.screenshot({
      path: test.info().outputPath(`error-${width}.png`),
      fullPage: true,
      caret: "initial",
    });
  });
}
