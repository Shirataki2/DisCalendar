// サイト全体で使う定数 (旧実装の siteconfig.js 相当)。
// メタ情報 (app/layout.tsx / app/manifest.ts) と LP・ヘッダ・フッタのリンク先をここに集める

export const SITE_NAME = "DisCalendar";

/** 公開 URL。OGP など絶対 URL が必要なメタ情報の基準 (旧 siteconfig.js の ogHost) */
export const SITE_URL = "https://discalendar.app";

export const SITE_DESCRIPTION =
  "DisCalendarはDiscord用のカレンダーアプリです。予定の作成から通知まで面倒なコマンド操作はほとんど必要ありません。" +
  "使い慣れたブラウザから、どこでも予定の追加や編集をすることができます。";

/** ブラウザの UI (アドレスバーなど) とマニフェストの theme_color (旧 siteconfig.js の themeColor) */
export const THEME_COLOR = "#303f9f";

/** サポートサーバー (Discord) の招待リンク */
export const SUPPORT_SERVER_URL = "https://discord.gg/MyaZRuze23";

export const GITHUB_URL = "https://github.com/Shirataki2/DisCalendarV3-new";

/**
 * サイト内のリンク先。docs (#6) と規約 (#7) は旧 URL を維持する予定なので、その URL に合わせてある
 * (ページ自体はそれぞれの Issue で追加される)
 */
export const ROUTES = {
  login: "/login",
  dashboard: "/dashboard",
  /** Bot の招待 URL へリダイレクトする (app/invite/route.ts) */
  invite: "/invite",
  docs: "/docs/gettingstarted",
  tos: "/support/tos",
  privacy: "/support/privacy",
} as const;
