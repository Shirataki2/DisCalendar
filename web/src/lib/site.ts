// サイト全体で使う定数 (旧実装の siteconfig.js 相当)。
// メタ情報 (app/layout.tsx / app/manifest.ts) と LP・ヘッダ・フッタのリンク先をここに集める

import type { Metadata } from "next";

export const SITE_NAME = "DisCalendar";

/** 本番の公開 URL。これ以外 (staging など) では検索エンジンに載せない (app/robots.ts) */
export const PRODUCTION_SITE_URL = "https://discalendar.app";

/**
 * 公開 URL。OGP など絶対 URL が必要なメタ情報の基準 (旧 siteconfig.js の ogHost)。
 * トップページは静的に生成されるので metadata は next build 時に確定し、実行時の環境変数では変えられない。
 * staging のように別ドメインで出すときは NEXT_PUBLIC_SITE_URL を渡してビルドし直す
 * (web/Dockerfile の ARG。未設定なら本番の URL)
 */
export const SITE_URL = process.env.NEXT_PUBLIC_SITE_URL ?? PRODUCTION_SITE_URL;

/** トップページの <title> (他のページは「<ページ名> | DisCalendar」)。探されそうな言葉を入れる */
export const SITE_TITLE = `${SITE_NAME} | Discord で予定を共有・通知できるカレンダー Bot`;

/** <meta name="description"> とマニフェストの説明。検索結果に出るので、何ができるかを先に書く */
export const SITE_DESCRIPTION =
  "DisCalendar は Discord サーバーで予定を共有できるカレンダー Bot です。" +
  "ブラウザのカレンダーから予定を作ると、開始時刻や事前のリマインダーを Discord のチャンネルに通知します。" +
  "コマンドを覚えなくても、スマホや PC から予定の追加・編集ができます。";

/** サイト共通の OGP 画像 (app/opengraph-image.png) の代替テキスト。app/opengraph-image.alt.txt と同じ文言にする */
export const OG_IMAGE_ALT = "DisCalendar - Discord 用予定管理 Bot";

type OpenGraph = NonNullable<Metadata["openGraph"]>;

/** 全ページの OGP に共通する項目 (app/layout.tsx と pageOpenGraph が使う) */
export const SITE_OPEN_GRAPH = {
  type: "website",
  locale: "ja_JP",
  siteName: SITE_NAME,
} satisfies OpenGraph;

/**
 * ページごとの OGP。metadata はセグメント間で浅くマージされ、子が openGraph を書くと app/layout.tsx の分も
 * app/opengraph-image.png の画像も丸ごと消えるので、共通の項目とサイト共通の画像をここで付け直す。
 * 同じセグメントに opengraph-image がある (共有ページなど) ときは `image: false` にしてその画像を使わせる
 */
export function pageOpenGraph({
  title,
  description,
  url,
  image = true,
}: {
  title: string;
  description: string;
  url: string;
  image?: boolean;
}): OpenGraph {
  return {
    ...SITE_OPEN_GRAPH,
    title,
    description,
    url,
    ...(image && {
      images: [
        {
          url: "/opengraph-image.png",
          width: 1200,
          height: 630,
          type: "image/png",
          alt: OG_IMAGE_ALT,
        },
      ],
    }),
  };
}

/**
 * ブランドカラー。ブラウザの UI (アドレスバーなど) とマニフェストの theme_color に使う
 * (旧 siteconfig.js の themeColor)。アイコンの地色 (scripts/brand/mark.svg) と同じ Discord blurple
 */
export const THEME_COLOR = "#5865F2";

/** サポートサーバー (Discord) の招待リンク */
export const SUPPORT_SERVER_URL = "https://discord.gg/MyaZRuze23";

export const GITHUB_URL = "https://github.com/Shirataki2/DisCalendar";

/** サイト内のリンク先。docs と規約 (利用規約 / プライバシーポリシー) は旧実装と同じ URL のまま */
export const ROUTES = {
  home: "/",
  login: "/login",
  /** ログイン不要の練習用カレンダー */
  tutorial: "/tutorial",
  dashboard: "/dashboard",
  /** 参加している全サーバーの予定をまとめて見る横断カレンダー (#98) */
  dashboardAll: "/dashboard/all",
  /** 管理コンソール (ADMIN_DISCORD_USER_IDS のユーザーのみ。それ以外は 404) */
  admin: "/admin",
  /** 管理コンソールの分析情報 (アクティブユーザー・予定の作成数の推移) */
  adminAnalytics: "/admin/analytics",
  /** 管理コンソールのギルド一覧 (`/admin/guilds/[id]` が詳細) */
  adminGuilds: "/admin/guilds",
  /** 管理コンソールの SQL コンソールと定型操作 */
  adminSql: "/admin/sql",
  /** 管理コンソールのユーザー・セッション管理 */
  adminUsers: "/admin/users",
  /** 管理コンソールの監査ログ */
  adminAuditLogs: "/admin/audit-logs",
  /** Bot の招待 URL へリダイレクトする (app/invite/route.ts)。サイト内のリンクは inviteHref で導入元を付ける */
  invite: "/invite",
  /** 支援 (ドネーション) のお願いページ */
  donation: "/donation",
  /** Stripe Payment Links へリダイレクトする (app/donation/checkout/route.ts) */
  donationCheckout: "/donation/checkout",
  docs: "/docs/gettingstarted",
  /** 更新履歴 (web/src/content/changelog.mdx) */
  changelog: "/changelog",
  /** 利用規約 (web/src/content/support/tos.mdx) */
  tos: "/support/tos",
  /** プライバシーポリシー (web/src/content/support/privacy.mdx) */
  privacy: "/support/privacy",
  /** 特定商取引法に基づく表記 (web/src/content/support/tokushoho.mdx) */
  tokushoho: "/support/tokushoho",
} as const;

/**
 * サイト内の招待リンクの導入元 (#374)。`/invite?from=<導入元>` でクリック数を導入元ごとに数える。
 * api が受け付ける値 (api/src/models/invite_clicks.rs の KNOWN_SOURCES) に無いものは other として数えられるので、
 * 増やすときは api 側にも足す
 */
export type InviteSource =
  | "lp-hero"
  | "lp-footer"
  | "docs-start"
  | "docs-invite"
  | "tutorial";

/** 導入元を付けた招待リンク */
export function inviteHref(source: InviteSource): string {
  return `${ROUTES.invite}?from=${source}`;
}
