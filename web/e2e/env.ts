import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { parseEnv } from "node:util";

// E2E の接続先とポート。playwright.config.ts (webServer の起動) と global-setup.ts (DB の初期化) で共有する。
// 通常の開発サーバー (web 3000 / api 8080) とは別のポートを使うので、dev を動かしたまま実行できる

export const WEB_DIR = path.resolve(__dirname, "..");
export const REPO_ROOT = path.resolve(WEB_DIR, "..");
/** ログイン済みブラウザの storageState (global-setup.ts が書き、use.storageState で読む)。git 管理外 */
export const STORAGE_STATE = path.join(WEB_DIR, "e2e/.auth/user.json");

export const WEB_PORT = portFromEnv("E2E_WEB_PORT", 3100);
export const API_PORT = portFromEnv("E2E_API_PORT", 8180);
/**
 * スクリーンショット撮影 (pnpm shot) では別のポートで待ち受ける。
 * web の Discord API 呼び出し (lib/discord.ts の /users/@me/guilds) は `next: { revalidate: 60 }` で
 * キャッシュされ、キーに URL が入る。ポートを分けておけば、撮影用に差し替えたギルド名 (fixtures.ts) が
 * 直後の pnpm e2e に出てきたり、その逆が起きたりしない
 */
export const DISCORD_MOCK_PORT = portFromEnv(
  "E2E_DISCORD_MOCK_PORT",
  process.env.E2E_SCREENSHOT === "1" ? 8191 : 8190,
);

export const WEB_URL = `http://localhost:${WEB_PORT}`;
export const API_URL = `http://127.0.0.1:${API_PORT}`;
export const DISCORD_MOCK_URL = `http://127.0.0.1:${DISCORD_MOCK_PORT}`;

/** Better Auth のセッション cookie の署名に使う (web と api に同じ値を渡す)。テスト専用のダミー値 */
export const BETTER_AUTH_SECRET = "e2e-better-auth-secret-not-for-production";

/** テスト用 DB 名に含まれていなければならない文字列 (開発 DB を誤って初期化しないための安全装置) */
const DATABASE_NAME_MARKER = "e2e";

/**
 * テスト用 DB の接続文字列。優先順:
 * 1. `E2E_DATABASE_URL`
 * 2. `web/.env.local` の `DATABASE_URL` の DB 名を `discalendar_e2e` に差し替えたもの (ローカル開発向け)
 * 3. `postgres://postgres:postgres@localhost:5432/discalendar_e2e` (CI の service container と同じ)
 *
 * DB は存在しなければ作られる (ensure-db.mjs)。初期化 (TRUNCATE) を伴うので DB 名に "e2e" を含むことを要求する
 */
export const DATABASE_URL = resolveDatabaseUrl();

function resolveDatabaseUrl(): string {
  const url =
    process.env.E2E_DATABASE_URL ??
    withDatabaseName(localDatabaseUrl(), "discalendar_e2e") ??
    "postgres://postgres:postgres@localhost:5432/discalendar_e2e";
  if (!databaseNameOf(url).includes(DATABASE_NAME_MARKER)) {
    throw new Error(
      `E2E_DATABASE_URL のデータベース名には "${DATABASE_NAME_MARKER}" を含めてください (テスト開始時に中身を消すため): ${databaseNameOf(url)}`,
    );
  }
  return url;
}

function localDatabaseUrl(): string | undefined {
  const file = path.join(WEB_DIR, ".env.local");
  if (!existsSync(file)) return undefined;
  return parseEnv(readFileSync(file, "utf8")).DATABASE_URL;
}

function withDatabaseName(
  url: string | undefined,
  name: string,
): string | undefined {
  if (!url) return undefined;
  const parsed = new URL(url);
  parsed.pathname = `/${name}`;
  return parsed.toString();
}

export function databaseNameOf(url: string): string {
  return decodeURIComponent(new URL(url).pathname.replace(/^\//, ""));
}

function portFromEnv(name: string, fallback: number): number {
  const raw = process.env[name];
  if (!raw) return fallback;
  const port = Number(raw);
  if (!Number.isInteger(port) || port <= 0) {
    throw new Error(`${name} はポート番号にしてください: ${raw}`);
  }
  return port;
}
