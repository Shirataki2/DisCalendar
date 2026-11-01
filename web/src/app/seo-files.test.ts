import { afterEach, describe, expect, it, vi } from "vitest";

// robots.ts / sitemap.ts は lib/site.ts の SITE_URL (ビルド時の NEXT_PUBLIC_SITE_URL) で出し分けるので、
// 環境変数を差し替えてからモジュールを読み直す
async function load(siteUrl: string | undefined) {
  vi.resetModules();
  if (siteUrl === undefined) vi.stubEnv("NEXT_PUBLIC_SITE_URL", undefined);
  else vi.stubEnv("NEXT_PUBLIC_SITE_URL", siteUrl);
  const robots = (await import("./robots")).default;
  const sitemap = (await import("./sitemap")).default;
  return { robots, sitemap };
}

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("robots.txt", () => {
  it("本番では公開しつつ API とリダイレクトだけを拒否し、sitemap を案内する", async () => {
    const { robots } = await load(undefined);
    const result = robots();
    expect(result.sitemap).toBe("https://discalendar.app/sitemap.xml");
    expect(result.rules).toMatchObject({ userAgent: "*", allow: "/" });
    const disallow = (result.rules as { disallow: string[] }).disallow;
    expect(disallow).toEqual(
      expect.arrayContaining(["/api/", "/invite", "/donation/checkout"]),
    );
    // noindex を読ませたいページは拒否しない
    expect(disallow).not.toContain("/dashboard");
    expect(disallow).not.toContain("/share");
    expect(disallow).toContain("/mcp$");
    for (const page of ["/mcp", "/mcp/connections", "/mcp/login"]) {
      expect(disallow).not.toContain(page);
    }
  });

  it("staging など本番以外の URL では全体を拒否する", async () => {
    const { robots } = await load("https://staging.discalendar.app");
    expect(robots()).toEqual({ rules: { userAgent: "*", disallow: "/" } });
  });
});

describe("sitemap.xml", () => {
  it("公開ページだけを絶対 URL で載せる", async () => {
    const { sitemap } = await load(undefined);
    const urls = sitemap().map((entry) => entry.url);
    expect(urls).toEqual(
      expect.arrayContaining([
        "https://discalendar.app/",
        "https://discalendar.app/tutorial",
        "https://discalendar.app/docs/gettingstarted",
        "https://discalendar.app/docs/commands",
        "https://discalendar.app/changelog",
        "https://discalendar.app/donation",
        "https://discalendar.app/support/privacy",
      ]),
    );
    expect(new Set(urls).size).toBe(urls.length);
    for (const url of urls) {
      expect(new URL(url).pathname).not.toMatch(
        /^\/(dashboard|admin|share|login|mcp|activity)/,
      );
    }
  });
});
