import { getMigrations } from "better-auth/db/migration";
import { auth } from "@/lib/auth";

// Better Auth のテーブル (user / session / account / verification) を DATABASE_URL の DB に作成・更新する。
// scripts/migrate.mjs (pnpm db:migrate) と同じことをアプリの起動時に行う (Docker 用、instrumentation.ts から呼ぶ)。
// auth.options を渡すので、auth.ts で plugin を足したときもその分のスキーマが対象になる
export async function migrateAuthSchema(): Promise<void> {
  const { toBeCreated, toBeAdded, runMigrations } = await getMigrations(
    auth.options,
  );
  if (toBeCreated.length === 0 && toBeAdded.length === 0) {
    console.log("[auth-migrate] schema is up to date");
    return;
  }
  for (const change of toBeCreated) {
    console.log(
      `[auth-migrate] create ${change.table}: ${Object.keys(change.fields).join(", ")}`,
    );
  }
  for (const change of toBeAdded) {
    console.log(
      `[auth-migrate] alter ${change.table}: add ${Object.keys(change.fields).join(", ")}`,
    );
  }
  await runMigrations();
  console.log("[auth-migrate] migration completed");
}
