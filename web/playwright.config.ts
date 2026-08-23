import { defineConfig, devices } from "@playwright/test";
import {
  API_PORT,
  API_URL,
  BETTER_AUTH_SECRET,
  DATABASE_URL,
  DISCORD_MOCK_URL,
  REPO_ROOT,
  STORAGE_STATE,
  WEB_DIR,
  WEB_PORT,
  WEB_URL,
} from "./e2e/env";
import { E2E_BOT_TOKEN } from "./e2e/fixtures";

// E2E テスト (Playwright)。実行: pnpm e2e (手順と環境変数は README の「テスト」)
//
// 起動するもの (webServer、この順に起動して待つ):
// 1. api (Rust)  … cargo run (CI は E2E_API_COMMAND でビルド済みバイナリ)。起動前に e2e/ensure-db.mjs で DB と Better Auth のテーブルを用意する
// 2. web (Next)  … ローカルは next dev、CI は next build + next start
// その後 e2e/global-setup.ts が Discord API のモックを起動し、DB を初期化してログイン済みの storageState を書く。
// web / api の Discord API の向き先 (DISCORD_API_BASE_URL) はそのモック。テストは Discord にもネットワークにも出ない

const CI = !!process.env.CI;

/** web / api に共通で渡す環境変数 (どちらの .env にある値も上書きする) */
const sharedEnv = {
  DATABASE_URL,
  BETTER_AUTH_SECRET,
  DISCORD_API_BASE_URL: DISCORD_MOCK_URL,
};

const apiCommand =
  process.env.E2E_API_COMMAND ?? "cargo run -p discalendar-api";
// CI では本番相当のビルドで動かす。output: "standalone" でも next start は動く (警告は出る。
// standalone の server.js は public/ と .next/static のコピーが要るので、ここでは使わない)
const webCommand =
  process.env.E2E_WEB_COMMAND ??
  (CI
    ? `pnpm exec next build && pnpm exec next start -p ${WEB_PORT}`
    : `pnpm exec next dev -p ${WEB_PORT}`);

export default defineConfig({
  testDir: "./e2e",
  globalSetup: "./e2e/global-setup.ts",
  // DB を共有するので並列にしない (テストごとに予定名を変えているが、ドラッグなどで他の予定が邪魔にならないように)
  fullyParallel: false,
  workers: 1,
  forbidOnly: CI,
  retries: CI ? 1 : 0,
  // next dev は初回アクセス時にコンパイルするので余裕を持たせる
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: CI ? [["github"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: WEB_URL,
    storageState: STORAGE_STATE,
    locale: "ja-JP",
    timezoneId: "Asia/Tokyo",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        // Playwright が期待する版の Chromium を入れられない環境 (Claude Code のクラウドセッションなど)
        // では、プリインストールの実行ファイルを E2E_CHROMIUM_PATH で指す
        launchOptions: process.env.E2E_CHROMIUM_PATH
          ? { executablePath: process.env.E2E_CHROMIUM_PATH }
          : undefined,
      },
    },
  ],
  webServer: [
    {
      name: "api",
      // DB が無いと api が起動できないので、先に作る (Better Auth のテーブルもここで作る)
      command: `node web/e2e/ensure-db.mjs && ${apiCommand}`,
      cwd: REPO_ROOT,
      url: `${API_URL}/healthz`,
      reuseExistingServer: !CI,
      // 初回は cargo build に時間がかかる
      timeout: 15 * 60 * 1000,
      stdout: "pipe",
      env: {
        ...sharedEnv,
        HOST: "127.0.0.1",
        PORT: String(API_PORT),
        DISCORD_BOT_TOKEN: E2E_BOT_TOKEN,
        ADMIN_DISCORD_USER_IDS: "",
        // query! のコンパイル時チェックは .sqlx/ のキャッシュを使う (空の E2E 用 DB に繋がせない)
        SQLX_OFFLINE: "true",
        // 起動ログと想定外のエラーだけ出す (テストが意図的に出す 401 / 403 も WARN で出る)
        RUST_LOG: "warn,discalendar_api=info",
      },
    },
    {
      name: "web",
      command: webCommand,
      cwd: WEB_DIR,
      url: WEB_URL,
      reuseExistingServer: !CI,
      timeout: 10 * 60 * 1000,
      stdout: "pipe",
      env: {
        ...sharedEnv,
        // 本番コンテナ・CI と同じサーバー日付にする (today.spec.ts の「今日」のセルの再現条件)
        TZ: "UTC",
        PORT: String(WEB_PORT),
        BETTER_AUTH_URL: WEB_URL,
        API_URL,
        // Discord OAuth はテストでは通らない (セッションは DB に直接作る) のでダミー
        DISCORD_CLIENT_ID: "e2e-discord-client-id",
        DISCORD_CLIENT_SECRET: "e2e-discord-client-secret",
        NEXT_TELEMETRY_DISABLED: "1",
      },
    },
  ],
});
