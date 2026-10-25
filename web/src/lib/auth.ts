import { betterAuth } from "better-auth";
import { nextCookies } from "better-auth/next-js";
import {
  activityTrustedOrigins,
  discordActivity,
} from "./activity/auth-plugin";
import { accountIssuerCompatibility } from "./auth-schema.mjs";
import { mcpEnabled } from "./mcp/config";
import { mcpPlugins } from "./mcp/plugins";
import { authPool } from "./mcp/store";

export const baseAuthOptions = {
  database: authPool,
  plugins: [accountIssuerCompatibility],
  socialProviders: {
    discord: {
      clientId: process.env.DISCORD_CLIENT_ID as string,
      clientSecret: process.env.DISCORD_CLIENT_SECRET as string,
      // デフォルトの identify, email に追加で指定される。
      // guilds はギルド選択画面でユーザーの所属サーバー一覧を取得するために必要
      scope: ["guilds"],
    },
  },
};

export const auth = betterAuth({
  ...baseAuthOptions,
  // Discord の Activity (#350) は discordsays.com のプロキシ経由で POST する
  trustedOrigins: activityTrustedOrigins(),
  plugins: [
    ...baseAuthOptions.plugins,
    discordActivity(),
    ...(mcpEnabled() ? mcpPlugins() : []),
    nextCookies(),
  ],
});

export type Session = typeof auth.$Infer.Session;
