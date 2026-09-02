// docs (使い方ページ) の一覧。旧実装の content/docs/*.md の frontmatter (title / description = 並び順) に相当する。
// 本文は src/content/docs/<slug>.mdx にあり、app/docs/[slug]/page.tsx がこの一覧を元に静的生成する。
// URL (/docs/<slug>) は旧実装のまま維持する

export interface DocPage {
  /** URL の末尾 (= src/content/docs/<slug>.mdx) */
  slug: string;
  /** 見出しとナビに出すタイトル */
  title: string;
  /** <meta name="description"> 用の要約 */
  description: string;
}

/** ナビと前後ページリンクの並び順 (導入の手順どおり) */
export const DOC_PAGES: readonly DocPage[] = [
  {
    slug: "gettingstarted",
    title: "基本的な使い方",
    description:
      "DisCalendar を使い始めるまでの 3 ステップ (Bot の招待、通知先チャンネルの設定、ブラウザからの予定作成)。",
  },
  {
    slug: "login",
    title: "ログイン",
    description:
      "Discord アカウントで DisCalendar にログインする手順と、ログイン時に求められる権限について。",
  },
  {
    slug: "invite",
    title: "Bot の招待",
    description:
      "DisCalendar の Bot を Discord サーバーに追加する手順と、Bot に必要な権限について。",
  },
  {
    slug: "initialize",
    title: "初期設定",
    description:
      "/init コマンドで予定の通知を受け取るチャンネルを設定する手順。",
  },
  {
    slug: "calendar",
    title: "予定の追加と表示",
    description:
      "ブラウザのカレンダー画面の見方、全サーバーの予定をまとめて見る「すべての予定」、予定ダイアログからの予定の作成 (日時・色・事前通知・説明) について。",
  },
  {
    slug: "edit",
    title: "予定の編集と削除",
    description:
      "作成した予定の編集・移動・削除の方法と、サーバー設定で編集できるユーザーを制限する方法。",
  },
  {
    slug: "commands",
    title: "利用可能なコマンド",
    description:
      "Discord から使えるスラッシュコマンド (/help, /create, /list, /init, /invite) の一覧と使い方。",
  },
];

export function findDocPage(slug: string): DocPage | undefined {
  return DOC_PAGES.find((page) => page.slug === slug);
}

/** 前後のページ (先頭・末尾では null) */
export function adjacentDocPages(slug: string): {
  prev: DocPage | null;
  next: DocPage | null;
} {
  const index = DOC_PAGES.findIndex((page) => page.slug === slug);
  return {
    prev: index > 0 ? DOC_PAGES[index - 1] : null,
    next:
      index >= 0 && index < DOC_PAGES.length - 1 ? DOC_PAGES[index + 1] : null,
  };
}

export function docPath(slug: string): `/docs/${string}` {
  return `/docs/${slug}`;
}
