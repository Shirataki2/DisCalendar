// E2E テストで使う固定データ。Discord モック (discord-mock.ts)・DB の初期データ (seed.ts)・テスト本体で共有する。
// ID は Snowflake 風の 18 桁 (api の is_snowflake を通す値) で、本物の Discord には存在しない

/** テストでログインしているユーザー (Better Auth の user + Discord アカウント) */
export const E2E_USER = {
  /** Better Auth の user.id */
  id: "e2e-user-1",
  name: "E2E User",
  email: "e2e-user@example.com",
  /** Discord のユーザー ID (account.accountId、api の AuthUser.discord_user_id) */
  discordId: "100000000000000001",
  /** account.accessToken。web が Discord モックの /users/@me/guilds を呼ぶときの Bearer トークン */
  accessToken: "e2e-discord-user-access-token",
} as const;

/** api が Discord モックを呼ぶときの Bot トークン (DISCORD_BOT_TOKEN) */
export const E2E_BOT_TOKEN = "e2e-discord-bot-token";

/**
 * テストに出てくるギルド。
 * - admin: ユーザーがオーナー (can_manage_server) で Bot 参加済み。予定の CRUD とサーバー設定の切替に使う
 * - member: ユーザーは一般メンバー (権限なし) で Bot 参加済み、restricted 設定済み。非管理者の表示に使う
 * - invitable: ユーザーに「サーバー管理」権限があるが Bot 未参加。サーバー選択画面の「Bot を招待できるサーバー」に出る
 */
export const E2E_GUILDS = {
  admin: {
    id: "200000000000000001",
    name: "E2E Admin Guild",
    /** Discord の /users/@me/guilds が返す permissions (ADMINISTRATOR) */
    permissions: "8",
    owner: true,
    botJoined: true,
  },
  member: {
    id: "200000000000000002",
    name: "E2E Member Guild",
    /** VIEW_CHANNEL のみ */
    permissions: "1024",
    owner: false,
    botJoined: true,
  },
  invitable: {
    id: "200000000000000003",
    name: "E2E Invitable Guild",
    /** MANAGE_GUILD (canInviteBot が true になる) */
    permissions: "32",
    owner: false,
    botJoined: false,
  },
} as const;

export type E2EGuild = (typeof E2E_GUILDS)[keyof typeof E2E_GUILDS];

/** member ギルドのオーナー (テストユーザーではない誰か) */
export const OTHER_OWNER_ID = "100000000000000099";
