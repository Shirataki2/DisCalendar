import type { MetadataRoute } from "next";
import { DOC_PAGES, docPath } from "@/lib/docs";
import { ROUTES, SITE_URL } from "@/lib/site";
import { SUPPORT_PAGES, supportPath } from "@/lib/support";
import { USECASE_PAGES, usecasePath } from "@/lib/usecases";

/**
 * /sitemap.xml。ログインせずに読める公開ページだけを載せる
 * (ダッシュボード・管理画面・共有ページ・ログイン画面は noindex なので含めない)。
 * 使い方・用途別の紹介・規約のページは一覧 (DOC_PAGES / USECASE_PAGES / SUPPORT_PAGES) から作るので、ページを足せば自動で載る
 */
export default function sitemap(): MetadataRoute.Sitemap {
  const url = (path: string) => new URL(path, SITE_URL).toString();
  return [
    { url: url(ROUTES.home), changeFrequency: "weekly", priority: 1 },
    { url: url(ROUTES.tutorial), changeFrequency: "monthly", priority: 0.8 },
    ...DOC_PAGES.map((page) => ({
      url: url(docPath(page.slug)),
      changeFrequency: "monthly" as const,
      priority: 0.7,
    })),
    ...USECASE_PAGES.map((page) => ({
      url: url(usecasePath(page.slug)),
      changeFrequency: "monthly" as const,
      priority: 0.7,
    })),
    { url: url(ROUTES.changelog), changeFrequency: "weekly", priority: 0.5 },
    { url: url(ROUTES.donation), changeFrequency: "yearly", priority: 0.3 },
    ...SUPPORT_PAGES.map((page) => ({
      url: url(supportPath(page.slug)),
      lastModified: page.updatedAt,
      changeFrequency: "yearly" as const,
      priority: 0.2,
    })),
  ];
}
