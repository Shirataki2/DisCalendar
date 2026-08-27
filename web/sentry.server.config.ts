// Next.js サーバー側 (Node.js runtime) のエラー収集 (#17)。src/instrumentation.ts から読み込まれる。
// DSN は実行時の環境変数で渡す (compose.yaml の SENTRY_DSN)。ブラウザ用に焼き込まれた
// NEXT_PUBLIC_SENTRY_DSN があればそれでもよい。未設定なら何も送らない
import * as Sentry from "@sentry/nextjs";

Sentry.init({
  dsn: process.env.SENTRY_DSN ?? process.env.NEXT_PUBLIC_SENTRY_DSN,
  // 未設定なら SDK の既定 (production)。staging では compose が SENTRY_ENVIRONMENT=staging を渡す
  environment: process.env.SENTRY_ENVIRONMENT,
  // 使うのはエラー収集だけ (instrumentation-client.ts と同じ方針)
  tracesSampleRate: 0,
  enableLogs: false,
  // 個人を識別しうる情報は送らない (プライバシーポリシーの説明と揃える)
  sendDefaultPii: false,
});
