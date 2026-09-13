import { expect, test } from "@playwright/test";

test.use({ storageState: { cookies: [], origins: [] } });

test("MCPのフォームPOSTはOriginを維持し、Refererにクエリを含めない", async ({
  page,
  request,
  baseURL,
}) => {
  const origin = new URL(baseURL ?? "").origin;
  // MCP全停止中も表示できるページで共通ヘッダを検証する。OAuthやDBへの書き込みは行わない。
  for (const action of ["/mcp/consent/submit", "/mcp/connections/revoke"]) {
    const response = await page.goto("/mcp/login?oauth_query=private-query");
    expect(response?.headers()["referrer-policy"]).toBe("strict-origin");
    await page.route(`**${action}`, (route) =>
      route.fulfill({ contentType: "text/html", body: "送信完了" }),
    );
    const posting = page.waitForRequest(
      (req) => new URL(req.url()).pathname === action,
    );
    await page.evaluate((target) => {
      const form = document.createElement("form");
      form.method = "POST";
      form.action = target;
      document.body.append(form);
      form.submit();
    }, action);
    const posted = await posting;
    expect(await posted.headerValue("origin")).toBe(origin);
    expect(await posted.headerValue("referer")).toBe(`${origin}/`);
    await page.waitForURL(`**${action}`);
  }
  // 検証を緩めてnullを許す修正になっていないことと、Route Handler側のヘッダも確認する。
  for (const value of [undefined, "null", "https://other.invalid"]) {
    const headers: Record<string, string> = value ? { Origin: value } : {};
    const response = await request.post("/mcp/connections/revoke", { headers });
    expect(response.status()).toBe(403);
    expect(response.headers()["referrer-policy"]).toBe("strict-origin");
  }
});
