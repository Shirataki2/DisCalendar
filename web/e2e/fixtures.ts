// E2E テストで使う固定データ。Discord モック (discord-mock.ts)・DB の初期データ (seed.ts)・テスト本体で共有する。
// ID は Snowflake 風の 18 桁 (api の is_snowflake を通す値) で、本物の Discord には存在しない

/**
 * スクリーンショット撮影 (pnpm shot) のときだけ true。
 * LP や使い方に載せる画像に "E2E Admin Guild" のような名前を写さないよう、表示名だけ差し替える。
 * globalSetup (Discord モックの起動と seed) が読むので、撮影用のテストが動き出す前に決まっている
 */
const SCREENSHOT = process.env.E2E_SCREENSHOT === "1";

/** テストで使う名前と、撮影のときに代わりに見せる名前 */
function displayName(normal: string, forScreenshot: string): string {
  return SCREENSHOT ? forScreenshot : normal;
}

/** テストでログインしているユーザー (Better Auth の user + Discord アカウント) */
export const E2E_USER = {
  /** Better Auth の user.id */
  id: "e2e-user-1",
  name: displayName("E2E User", "user"),
  email: "e2e-user@example.com",
  /** Discord のユーザー ID (account.accountId、api の AuthUser.discord_user_id) */
  discordId: "100000000000000001",
  /** account.accessToken。web が Discord モックの /users/@me/guilds を呼ぶときの Bearer トークン */
  accessToken: "e2e-discord-user-access-token",
} as const;

/** api が Discord モックを呼ぶときの Bot トークン (DISCORD_BOT_TOKEN) */
export const E2E_BOT_TOKEN = "e2e-discord-bot-token";

/** Bot 自身の Discord ユーザー ID (`GET /users/@me`。api が Bot の権限計算に使う) */
export const E2E_BOT_USER_ID = "100000000000000050";

/** botManageEvents のギルドで Bot に付いているロール (「イベントの管理」を持つ) */
export const E2E_BOT_ROLE_ID = "300000000000000001";

/** テストに出てくるギルド (Discord モックの応答と guilds テーブルの行のもと) */
export interface E2EGuild {
  /** Discord のギルド ID (Snowflake。API 境界では必ず文字列) */
  id: string;
  name: string;
  /** Discord の /users/@me/guilds が返す permissions */
  permissions: string;
  /** テストユーザーがオーナーか */
  owner: boolean;
  /** Bot が参加しているか (guilds テーブルに行が入る) */
  botJoined: boolean;
  /** Bot 自身が「イベントの管理」権限を持つか (#94。Discord モックのロール応答に反映される) */
  botManageEvents: boolean;
}

/**
 * テストが参照するギルド。
 * - admin: ユーザーがオーナー (can_manage_server) で Bot 参加済み。予定の CRUD とサーバー設定の切替に使う
 * - member: ユーザーは一般メンバー (権限なし) で Bot 参加済み、restricted 設定済み。非管理者の表示に使う
 * - invitable: ユーザーに「サーバー管理」権限があるが Bot 未参加。サーバー選択画面の「Bot を招待できるサーバー」に出る
 * - noEventsPerm: Bot 参加済みだが「イベントの管理」権限がない。Discord 連携 (#94) の無効化表示に使う
 */
export const E2E_GUILDS = {
  admin: {
    id: "200000000000000001",
    name: displayName("E2E Admin Guild", "ゲーム部"),
    /** ADMINISTRATOR */
    permissions: "8",
    owner: true,
    botJoined: true,
    botManageEvents: true,
  },
  member: {
    id: "200000000000000002",
    name: displayName("E2E Member Guild", "DisCalendar サポート"),
    /** VIEW_CHANNEL のみ */
    permissions: "1024",
    owner: false,
    botJoined: true,
    botManageEvents: true,
  },
  invitable: {
    id: "200000000000000003",
    name: displayName("E2E Invitable Guild", "バンドサークル"),
    /** MANAGE_GUILD (canInviteBot が true になる) */
    permissions: "32",
    owner: false,
    botJoined: false,
    botManageEvents: false,
  },
  noEventsPerm: {
    id: "200000000000000006",
    name: displayName("E2E No Events Guild", "写真部"),
    /** VIEW_CHANNEL のみ (restricted ではないので予定の編集はできる) */
    permissions: "1024",
    owner: false,
    botJoined: true,
    botManageEvents: false,
  },
} satisfies Record<string, E2EGuild>;

/**
 * サーバー選択の画像を実際の使用感に近づけるため、撮影のときだけ一覧に足すギルド。
 * テストからは参照しない (テストの前提を増やさないため)
 */
const SCREENSHOT_GUILDS: E2EGuild[] = SCREENSHOT
  ? [
      {
        id: "200000000000000004",
        name: "研究室",
        permissions: "1024",
        owner: false,
        botJoined: true,
        botManageEvents: true,
      },
      {
        id: "200000000000000005",
        name: "読書会",
        permissions: "32",
        owner: false,
        botJoined: false,
        botManageEvents: false,
      },
    ]
  : [];

/** Discord モックが返し、seed が DB に入れるギルドの全体 */
export const E2E_ALL_GUILDS: E2EGuild[] = [
  ...Object.values(E2E_GUILDS),
  ...SCREENSHOT_GUILDS,
];

/** member ギルドのオーナー (テストユーザーではない誰か) */
export const OTHER_OWNER_ID = "100000000000000099";
