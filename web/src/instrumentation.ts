// Next.js サーバーの起動時に一度だけ呼ばれる (next build 中や Edge runtime でも呼ばれるので条件で絞る)。
// AUTO_MIGRATE=true のとき (web/Dockerfile の既定) に Better Auth のテーブルを作成・更新してからリクエストを受け付ける。
// ローカル開発では従来どおり pnpm db:migrate を使う
export async function register(): Promise<void> {
  if (process.env.NEXT_RUNTIME !== "nodejs") return;
  if (process.env.AUTO_MIGRATE !== "true") return;
  const { migrateAuthSchema } = await import("@/lib/auth-migrate");
  await migrateAuthSchema();
}
