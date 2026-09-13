import { betterAuth } from "better-auth";
import { nextCookies } from "better-auth/next-js";
import { mcpEnabled } from "./mcp/config";
import { mcpPlugins } from "./mcp/plugins";
import { authPool } from "./mcp/store";

export const baseAuthOptions = {
  database: authPool,
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
  plugins: [...(mcpEnabled() ? mcpPlugins() : []), nextCookies()],
});

export type Session = typeof auth.$Infer.Session;
