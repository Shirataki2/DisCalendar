import { createHmac, randomBytes } from "node:crypto";
import { Pool } from "pg";
import { E2E_ALL_GUILDS, E2E_GUILDS, E2E_USER } from "./fixtures";

// E2E 用 DB の初期データ。Discord OAuth を通さずにログイン済みの状態を作るため、
// Better Auth のテーブル (user / account / session) に直接行を入れ、api が読む guilds / guild_config も用意する。
// テーブルは api (起動時のマイグレーション) と ensure-db.mjs (Better Auth) が作っている前提

/** Better Auth のセッション cookie 名 (baseURL が http なので `__Secure-` は付かない) */
export const SESSION_COOKIE_NAME = "better-auth.session_token";

/**
 * Better Auth (better-call の signCookieValue) と同じ形の署名付き cookie 値:
 * `encodeURIComponent("<token>.<base64(HMAC-SHA256(secret, token))>")`。
 * api/src/auth.rs はこの署名を検証してから session テーブルを引く
 */
export function signSessionCookie(token: string, secret: string): string {
  const signature = createHmac("sha256", secret).update(token).digest("base64");
  return encodeURIComponent(`${token}.${signature}`);
}

/** Playwright の storageState / addCookies に渡す形のセッション cookie */
export function sessionCookie(token: string, secret: string) {
  return {
    name: SESSION_COOKIE_NAME,
    value: signSessionCookie(token, secret),
    domain: "localhost",
    path: "/",
    expires: -1,
    httpOnly: true,
    secure: false,
    sameSite: "Lax" as const,
  };
}

/**
 * テストユーザーのセッションをもう 1 つ作ってトークンを返す
 * (ログアウトのテストなど、共有のセッションを壊したくないときに使う)
 */
export async function createExtraSession(databaseUrl: string): Promise<string> {
  const pool = new Pool({ connectionString: databaseUrl });
  try {
    const token = randomBytes(32).toString("hex");
    await pool.query(
      `INSERT INTO "session" (id, "expiresAt", token, "createdAt", "updatedAt", "ipAddress", "userAgent", "userId")
       VALUES ($1, now() + interval '1 day', $2, now(), now(), NULL, 'playwright', $3)`,
      [`e2e-session-${token.slice(0, 8)}`, token, E2E_USER.id],
    );
    return token;
  } finally {
    await pool.end();
  }
}

/** DB を空にしてテストデータを入れ、セッショントークン (cookie に入れる前の値) を返す */
export async function seedDatabase(databaseUrl: string): Promise<string> {
  const pool = new Pool({ connectionString: databaseUrl });
  try {
    const tables = await pool.query<{ table_name: string }>(
      `SELECT table_name FROM information_schema.tables
       WHERE table_schema = 'public' AND table_name IN ('guilds', 'guild_config', 'events', 'user', 'session', 'account')`,
    );
    const found = new Set(tables.rows.map((r) => r.table_name));
    for (const table of [
      "guilds",
      "guild_config",
      "events",
      "user",
      "session",
      "account",
    ]) {
      if (!found.has(table)) {
        throw new Error(
          `テーブル ${table} がありません。api のマイグレーション (起動時) と ensure-db.mjs (Better Auth) が ${databaseUrl} に適用されているか確認してください`,
        );
      }
    }

    const token = randomBytes(32).toString("hex");
    const client = await pool.connect();
    try {
      await client.query("BEGIN");
      // 前回の実行で残ったものを消す (旧 Bot と共有するスキーマでも、ここは E2E 専用 DB)
      await client.query(
        "TRUNCATE events, guild_config, guilds RESTART IDENTITY",
      );
      await client.query('DELETE FROM "session"');
      await client.query('DELETE FROM "account"');
      await client.query('DELETE FROM "user"');

      await client.query(
        `INSERT INTO "user" (id, name, email, "emailVerified", image, "createdAt", "updatedAt")
         VALUES ($1, $2, $3, true, NULL, now(), now())`,
        [E2E_USER.id, E2E_USER.name, E2E_USER.email],
      );
      // Discord 連携アカウント。accessToken は平文 (account.encryptOAuthTokens は未設定) で、
      // 期限が先なので Better Auth の getAccessToken は refresh せずこの値を返す
      await client.query(
        `INSERT INTO "account" (id, issuer, "accountId", "providerId", "userId", "accessToken", "refreshToken",
                                "accessTokenExpiresAt", scope, "createdAt", "updatedAt")
         VALUES ($1, 'local:oauth:discord', $2, 'discord', $3, $4, NULL, now() + interval '1 year',
                 'identify,email,guilds', now(), now())`,
        [
          "e2e-account-1",
          E2E_USER.discordId,
          E2E_USER.id,
          E2E_USER.accessToken,
        ],
      );
      await client.query(
        `INSERT INTO "session" (id, "expiresAt", token, "createdAt", "updatedAt", "ipAddress", "userAgent", "userId")
         VALUES ($1, now() + interval '7 days', $2, now(), now(), NULL, 'playwright', $3)`,
        ["e2e-session-1", token, E2E_USER.id],
      );

      // Bot が参加しているギルド (bot/ が guilds テーブルに書く内容の代わり)
      for (const guild of E2E_ALL_GUILDS) {
        if (!guild.botJoined) continue;
        await client.query(
          "INSERT INTO guilds (guild_id, name, avatar_url, locale) VALUES ($1, $2, NULL, 'ja')",
          [guild.id, guild.name],
        );
      }
      // member ギルドは restricted モード (非管理者の表示を確認する)。admin ギルドは明示的に false から始める
      await client.query(
        "INSERT INTO guild_config (guild_id, restricted) VALUES ($1, false), ($2, true)",
        [E2E_GUILDS.admin.id, E2E_GUILDS.member.id],
      );
      await client.query("COMMIT");
    } catch (error) {
      await client.query("ROLLBACK");
      throw error;
    } finally {
      client.release();
    }
    return token;
  } finally {
    await pool.end();
  }
}
