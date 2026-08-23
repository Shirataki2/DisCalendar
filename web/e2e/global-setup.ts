import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { FullConfig } from "@playwright/test";
import { startDiscordMock } from "./discord-mock";
import {
  BETTER_AUTH_SECRET,
  DATABASE_URL,
  DISCORD_MOCK_PORT,
  STORAGE_STATE,
} from "./env";
import { seedDatabase, sessionCookie } from "./seed";

/**
 * 全テストの前に 1 回だけ行うこと (Playwright は webServer の起動後にこれを呼ぶ):
 * 1. Discord API のモックを起動する (web / api の DISCORD_API_BASE_URL の向き先)
 * 2. DB を初期化してテストユーザー・ギルドを入れる
 * 3. セッション cookie を storageState に書き出し、各テストがログイン済みの状態で始まるようにする
 */
export default async function globalSetup(_config: FullConfig) {
  const mock = await startDiscordMock(DISCORD_MOCK_PORT);

  const token = await seedDatabase(DATABASE_URL);
  mkdirSync(path.dirname(STORAGE_STATE), { recursive: true });
  writeFileSync(
    STORAGE_STATE,
    JSON.stringify(
      { cookies: [sessionCookie(token, BETTER_AUTH_SECRET)], origins: [] },
      null,
      2,
    ),
  );

  return async () => {
    await new Promise<void>((resolve) => mock.close(() => resolve()));
  };
}
