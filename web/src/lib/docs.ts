// docs (使い方ページ) の一覧。旧実装の content/docs/*.md の frontmatter (title / description = 並び順) に相当する。
// 本文は src/content/docs/<slug>.mdx にあり、app/docs/[slug]/page.tsx がこの一覧を元に静的生成する。
// URL (/docs/<slug>) は旧実装のまま維持する

export interface DocPage {
  /** URL の末尾 (= src/content/docs/<slug>.mdx) */
  slug: string;
  /** 見出しとナビに出すタイトル */
  title: string;
  /**
   * <title> と OGP のタイトル (省略時は title)。ナビの短い見出しだけでは何のページか検索結果で伝わらないとき、
   * 探されそうな言葉 (Discord・カレンダー・通知など) を補う。画面には出さないので翻訳しない
   */
  metaTitle?: string;
  /** <meta name="description"> 用の要約 (見出しの下にも出す) */
  description: string;
}

/** ナビと前後ページリンクの並び順 (導入の手順どおり) */
export const DOC_PAGES: readonly DocPage[] = [
  {
    slug: "gettingstarted",
    title: "基本的な使い方",
    metaTitle: "Discord で予定を共有する基本的な使い方",
    description:
      "Discord サーバーで予定を共有・通知できる DisCalendar を使い始めるまでの 3 ステップ (Bot の招待、通知先チャンネルの設定、ブラウザからの予定作成)。",
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
    metaTitle: "Discord サーバーへの Bot の招待",
    description:
      "DisCalendar の Bot を Discord サーバーに追加する手順と、Bot に必要な権限について。",
  },
  {
    slug: "initialize",
    title: "初期設定",
    metaTitle: "予定の通知先チャンネルの初期設定",
    description:
      "Web のサーバー設定または /init コマンドで、予定の開始やリマインダー (事前通知) を受け取るチャンネルを設定する手順。",
  },
  {
    slug: "calendar",
    title: "予定の追加と表示",
    metaTitle: "ブラウザのカレンダーで予定を追加・表示する",
    description:
      "ブラウザのカレンダー画面の見方、全サーバーの予定をまとめて見る「すべての予定」、予定ダイアログからの予定の作成 (日時・色・事前通知・説明) について。",
  },
  {
    slug: "activity",
    title: "Discord の中で開く",
    metaTitle: "Discord の中でカレンダーを開く",
    description:
      "Discord のアクティビティとして、ボイスチャンネルやテキストチャンネルからサーバーのカレンダーを開く方法。",
  },
  {
    slug: "polls",
    title: "日程調整",
    metaTitle: "Discord で候補日の回答を集める日程調整",
    description: "候補日への回答を集め、選んだ日時で予定を作成する方法。",
  },
  {
    slug: "edit",
    title: "予定の編集と削除",
    metaTitle: "Discord の予定の編集と削除",
    description:
      "作成した予定の編集・移動・削除の方法と、サーバー設定で編集できるユーザーを制限する方法。",
  },
  {
    slug: "subscribe",
    title: "外部カレンダーで見る",
    metaTitle: "Google カレンダー・iPhone などの外部カレンダーで見る",
    description:
      "サーバーの予定を Google カレンダー・Apple カレンダー・Outlook などで購読する方法と、フィード URL の発行・再発行・無効化について。",
  },
  {
    slug: "webhooks",
    title: "予定の変更を外部に通知する",
    description:
      "Webhook の登録、Discord への通知、JSON 本文と署名の検証方法。",
  },
  {
    slug: "mcp",
    title: "Codexとの接続",
    description:
      "CodexとのMCP接続で予定を確認・作成・編集する方法。権限の選択、接続解除と困ったときの対応。",
  },
  {
    slug: "commands",
    title: "利用可能なコマンド",
    metaTitle: "Discord のスラッシュコマンド一覧",
    description:
      "Discord から使えるスラッシュコマンド (/help, /create, /list, /search, /init, /invite) の一覧と使い方。",
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
