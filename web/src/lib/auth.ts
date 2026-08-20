import { betterAuth } from "better-auth";
import { nextCookies } from "better-auth/next-js";
import { Pool } from "pg";

export const auth = betterAuth({
  database: new Pool({ connectionString: process.env.DATABASE_URL }),
  socialProviders: {
    discord: {
      clientId: process.env.DISCORD_CLIENT_ID as string,
      clientSecret: process.env.DISCORD_CLIENT_SECRET as string,
      // デフォルトの identify, email に追加で指定される。
      // guilds はギルド選択画面でユーザーの所属サーバー一覧を取得するために必要
      scope: ["guilds"],
    },
  },
  plugins: [nextCookies()],
});

export type Session = typeof auth.$Infer.Session;
