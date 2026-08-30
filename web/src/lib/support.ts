// 規約ページ (利用規約 / プライバシーポリシー) の一覧。docs (lib/docs.ts) と同じ考え方で、
// 本文は src/content/support/<slug>.mdx にあり、app/support/[slug]/page.tsx がこの一覧を元に静的生成する。
// URL (/support/tos, /support/privacy) は旧実装 (@nuxt/content の content/support/*.md) のまま維持する

export interface SupportPage {
  /** URL の末尾 (= src/content/support/<slug>.mdx) */
  slug: string;
  /** 見出しとメタ情報に出すタイトル */
  title: string;
  /** <meta name="description"> 用の要約 */
  description: string;
  /** 本文の最終更新日 (YYYY-MM-DD)。本文を直したらここも直す */
  updatedAt: string;
}

export const SUPPORT_PAGES: readonly SupportPage[] = [
  {
    slug: "tos",
    title: "利用規約",
    description:
      "DisCalendar を利用するうえでの条件・禁止事項・免責事項を定めた利用規約です。",
    updatedAt: "2026-08-23",
  },
  {
    slug: "privacy",
    title: "プライバシーポリシー",
    description:
      "DisCalendar が取得する情報 (Discord アカウントの情報、サーバーと予定の情報) と、その利用目的・管理方法を定めたプライバシーポリシーです。",
    updatedAt: "2026-08-23",
  },
  {
    slug: "tokushoho",
    title: "特定商取引法に基づく表記",
    description:
      "DisCalendar への支援 (ドネーション) の決済に関する特定商取引法に基づく表記です。",
    updatedAt: "2026-08-31",
  },
];

export function findSupportPage(slug: string): SupportPage | undefined {
  return SUPPORT_PAGES.find((page) => page.slug === slug);
}

export function supportPath(slug: string): `/support/${string}` {
  return `/support/${slug}`;
}

/** 規約の慣習に合わせて「2026年8月23日」の形にする */
export function formatUpdatedAt(updatedAt: string): string {
  const [year, month, day] = updatedAt.split("-").map(Number);
  return `${year}年${month}月${day}日`;
}
