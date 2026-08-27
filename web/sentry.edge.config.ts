// Edge runtime 用のエラー収集 (#17)。今は Edge runtime のルートは無いが、instrumentation は
// Edge でも呼ばれる (src/instrumentation.ts のコメント参照) ので、増えたときに漏れないよう置いておく
import * as Sentry from "@sentry/nextjs";

Sentry.init({
  dsn: process.env.SENTRY_DSN ?? process.env.NEXT_PUBLIC_SENTRY_DSN,
  environment: process.env.SENTRY_ENVIRONMENT,
  tracesSampleRate: 0,
  enableLogs: false,
  sendDefaultPii: false,
});
