// Better Auth のテーブルを DATABASE_URL の DB に作成/更新する。
// @better-auth/cli はランタイム (better-auth 1.7.x) より古いスキーマを生成するため、
// インストール済みパッケージ同梱の migration エンジンを直接使う。
// 実行: pnpm db:migrate
import { getMigrations } from "better-auth/db/migration";
import { Pool } from "pg";

const url = process.env.DATABASE_URL;
if (!url) {
  console.error("DATABASE_URL is not set (use: node --env-file=.env.local)");
  process.exit(1);
}

const pool = new Pool({ connectionString: url });
const { toBeCreated, toBeAdded, runMigrations } = await getMigrations({
  database: pool,
});

if (toBeCreated.length === 0 && toBeAdded.length === 0) {
  console.log("schema is up to date");
} else {
  for (const change of toBeCreated) {
    console.log(`create ${change.table}: ${Object.keys(change.fields).join(", ")}`);
  }
  for (const change of toBeAdded) {
    console.log(`alter ${change.table}: add ${Object.keys(change.fields).join(", ")}`);
  }
  await runMigrations();
  console.log("migration completed");
}
await pool.end();
