// ブラウザ側のエラー収集 (#17)。React の hydration より前に一度だけ実行される。
// DSN は next build 時に焼き込まれる (web/Dockerfile の ARG NEXT_PUBLIC_SENTRY_DSN)。
// 未設定なら Sentry は無効で何も送らない (ローカル開発・CI・E2E)
import * as Sentry from "@sentry/nextjs";
import { SITE_URL } from "@/lib/site";

// Sentry 上の environment タグ。web はビルドが staging / 本番でドメインごとに分かれている (#87) ので、
// 焼き込まれた SITE_URL から導出する (サーバー側の SENTRY_ENVIRONMENT に相当する値をブラウザは持てない)
function sentryEnvironment(): string {
  const host = new URL(SITE_URL).hostname;
  if (host === "discalendar.app") return "production";
  if (host.startsWith("staging.")) return "staging";
  return "development";
}

Sentry.init({
  dsn: process.env.NEXT_PUBLIC_SENTRY_DSN,
  environment: sentryEnvironment(),
  // 使うのはエラー収集だけ。パフォーマンス計測・Session Replay・Logs は無効にして
  // 無料枠を errors 以外で消費しない
  tracesSampleRate: 0,
  enableLogs: false,
  // IP アドレスや cookie などの個人を識別しうる情報は送らない (SDK の既定だが、
  // プライバシーポリシー「不具合 (エラー) の記録」の説明と揃えるために明示する)
  sendDefaultPii: false,
});

// App Router のページ遷移をパンくずとして記録する (エラー時に直前の遷移が分かる)
export const onRouterTransitionStart = Sentry.captureRouterTransitionStart;
