// E2E 用 DB の用意。playwright.config.ts が api を起動する直前に実行する
// (api は起動時に DATABASE_URL へ接続してマイグレーションを適用するので、その前に DB そのものが要る)。
//
// 1. DATABASE_URL のデータベースがなければ作る (同じサーバーの postgres DB に繋いで CREATE DATABASE)
// 2. Better Auth のテーブル (user / session / account / verification) を作る (scripts/migrate.mjs と同じ)
//
// Playwright の webServer.command から素の node で動かすため、TypeScript ではなく .mjs にしている
import { getMigrations } from "better-auth/db/migration";
import { Client, Pool } from "pg";

const url = process.env.DATABASE_URL;
if (!url) {
  console.error("[e2e] DATABASE_URL is not set");
  process.exit(1);
}

await ensureDatabase(url);
await migrateBetterAuth(url);

async function ensureDatabase(databaseUrl) {
  const parsed = new URL(databaseUrl);
  const name = decodeURIComponent(parsed.pathname.replace(/^\//, ""));
  // 管理用に同じサーバーの postgres DB へ繋ぐ
  parsed.pathname = "/postgres";
  const client = new Client({ connectionString: parsed.toString() });
  await client.connect();
  try {
    const { rowCount } = await client.query(
      "SELECT 1 FROM pg_database WHERE datname = $1",
      [name],
    );
    if (rowCount === 0) {
      // 識別子はバインドできないので引用符で囲む (name は .env 由来で信頼できる前提)
      await client.query(`CREATE DATABASE "${name.replaceAll('"', '""')}"`);
      console.log(`[e2e] created database ${name}`);
    }
  } finally {
    await client.end();
  }
}

async function migrateBetterAuth(databaseUrl) {
  const pool = new Pool({ connectionString: databaseUrl });
  try {
    const { toBeCreated, toBeAdded, runMigrations } = await getMigrations({
      database: pool,
    });
    if (toBeCreated.length > 0 || toBeAdded.length > 0) {
      await runMigrations();
      console.log("[e2e] Better Auth schema migrated");
    }
  } finally {
    await pool.end();
  }
}
