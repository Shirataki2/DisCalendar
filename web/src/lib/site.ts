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

/** サイト内のリンク先。docs と規約 (利用規約 / プライバシーポリシー) は旧実装と同じ URL のまま */
export const ROUTES = {
  login: "/login",
  dashboard: "/dashboard",
  /** 管理コンソール (ADMIN_DISCORD_USER_IDS のユーザーのみ。それ以外は 404) */
  admin: "/admin",
  /** 管理コンソールのギルド一覧 (`/admin/guilds/[id]` が詳細) */
  adminGuilds: "/admin/guilds",
  /** 管理コンソールの SQL コンソールと定型操作 */
  adminSql: "/admin/sql",
  /** 管理コンソールのユーザー・セッション管理 */
  adminUsers: "/admin/users",
  /** 管理コンソールの監査ログ */
  adminAuditLogs: "/admin/audit-logs",
  /** Bot の招待 URL へリダイレクトする (app/invite/route.ts) */
  invite: "/invite",
  docs: "/docs/gettingstarted",
  /** 利用規約 (web/src/content/support/tos.mdx) */
  tos: "/support/tos",
  /** プライバシーポリシー (web/src/content/support/privacy.mdx) */
  privacy: "/support/privacy",
} as const;
