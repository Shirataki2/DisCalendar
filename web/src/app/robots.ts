import type { MetadataRoute } from "next";
import { PRODUCTION_SITE_URL, ROUTES, SITE_URL } from "@/lib/site";

/**
 * /robots.txt。本番では公開ページを検索エンジンに載せ、sitemap の場所を伝える。
 * staging など本番以外の URL でビルドしたときは、本番と同じ内容が検索結果に出ないよう全体を拒否する。
 *
 * ダッシュボード・管理画面・共有ページなどは各ページの noindex で検索結果から外す。
 * ここで拒否するとクローラーが noindex を読めなくなるので、HTML を返さない URL (API・リダイレクト) だけを拒否する
 */
export default function robots(): MetadataRoute.Robots {
  if (SITE_URL !== PRODUCTION_SITE_URL) {
    return { rules: { userAgent: "*", disallow: "/" } };
  }
  return {
    rules: {
      userAgent: "*",
      allow: "/",
      disallow: [
        "/api/",
        // MCP は本体 (/mcp) と HTML を返さないハンドラーだけ。接続管理・ログイン・同意の画面は noindex を読ませる
        // ($ は URL の末尾。Google・Bing が対応している)
        "/mcp$",
        "/mcp/introspect",
        "/mcp/login/start",
        "/mcp/consent/submit",
        "/mcp/connections/revoke",
        "/serwist/",
        ROUTES.invite,
        ROUTES.donationCheckout,
      ],
    },
    sitemap: `${SITE_URL}/sitemap.xml`,
  };
}
