// Next.js サーバーの起動時に一度だけ呼ばれる (next build 中や Edge runtime でも呼ばれるので条件で絞る)。
// - Sentry のサーバー側初期化 (#17)。DSN 未設定なら何も送らない
// - AUTO_MIGRATE=true のとき (web/Dockerfile の既定) に Better Auth のテーブルを作成・更新してから
//   リクエストを受け付ける。ローカル開発では従来どおり pnpm db:migrate を使う
import * as Sentry from "@sentry/nextjs";

export async function register(): Promise<void> {
  if (process.env.NEXT_RUNTIME === "edge") {
    await import("../sentry.edge.config");
    return;
  }
  if (process.env.NEXT_RUNTIME !== "nodejs") return;
  await import("../sentry.server.config");
  if (process.env.AUTO_MIGRATE !== "true") return;
  const { migrateAuthSchema } = await import("@/lib/auth-migrate");
  await migrateAuthSchema();
}

// Server Component・Route Handler・Server Action のエラーを Sentry へ送る
export const onRequestError = Sentry.captureRequestError;
