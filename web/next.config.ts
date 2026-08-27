import createMDX from "@next/mdx";
import { withSentryConfig } from "@sentry/nextjs";
import { withSerwist } from "@serwist/turbopack";
import type { NextConfig } from "next";

// Rust API (api/) の URL。rewrites はサーバー側で解決されるので、この値がブラウザに出ることはない
const API_URL = process.env.API_URL ?? "http://127.0.0.1:8080";

const nextConfig: NextConfig = {
  // Docker 用に .next/standalone/ へ実行に必要なファイルだけを出力する (web/Dockerfile)
  output: "standalone",
  // Node 22.12+ の require() は @swc/helpers の exports の "module-sync" 条件で ESM 側 (esm/*.js) を選ぶが、
  // Next のファイルトレースは CJS 側しか standalone にコピーしないため、起動時に
  // "Cannot find module '@swc/helpers/esm/_interop_require_default.js'" で落ちる (vercel/next.js#93852 と同系統)。
  // pnpm の実体ディレクトリから ESM 側も standalone に含める
  outputFileTracingIncludes: {
    "/*": [
      "./node_modules/.pnpm/@swc+helpers@*/node_modules/@swc/helpers/esm/**",
    ],
  },
  async headers() {
    return [
      // Service Worker (app/serwist/[path]/route.ts) は静的に生成されるため Next が s-maxage=31536000 を付けるが、
      // URL が変わらない sw.js を CDN (Cloudflare) に長期間キャッシュされると、デプロイしても古い SW が配られ続ける。
      // 常に再検証させる (ブラウザは SW スクリプトの取得で HTTP キャッシュを元々バイパスする)
      {
        source: "/serwist/:path*",
        headers: [{ key: "Cache-Control", value: "no-cache" }],
      },
    ];
  },
  async redirects() {
    return [
      // docs の入口。旧実装にも /docs 自体のページは無く、最初の記事 (/docs/gettingstarted) が入口だった
      {
        source: "/docs",
        destination: "/docs/gettingstarted",
        permanent: false,
      },
    ];
  },
  async rewrites() {
    return [
      // ブラウザは同一オリジンの /local/api/* を叩き、Next.js が Rust API へプロキシする
      // (旧実装の @nuxtjs/proxy と同じ構成)。Better Auth のセッション cookie が
      // そのまま転送されるので、API 側はそれを検証して認証する (api/src/auth.rs)
      {
        source: "/local/api/:path*",
        destination: `${API_URL}/:path*`,
      },
    ];
  },
};

// docs (使い方ページ) は src/content/docs/*.mdx を app/docs/[slug]/page.tsx から import して描画する
// (旧実装の @nuxt/content 相当。mdx-components.tsx で見出しや画像の描画を差し替える)。
// 表を書けるように remark-gfm を入れる。Turbopack にプラグインを渡すときは関数ではなくパッケージ名の文字列で指定する
const withMDX = createMDX({
  options: {
    remarkPlugins: ["remark-gfm"],
  },
});

// Service Worker (app/serwist/[path]/route.ts が app/sw.ts を esbuild で束ねる)。
// withSerwist は esbuild を serverExternalPackages に足すだけで、SW の生成は Route Handler 側で行う
//
// withSentryConfig はエラー監視 (#17) のビルド設定。SDK の初期化は src/instrumentation.ts (サーバー) と
// src/instrumentation-client.ts (ブラウザ) で行い、DSN 未設定なら何も送らない。
// ソースマップのアップロードは SENTRY_AUTH_TOKEN / SENTRY_ORG / SENTRY_PROJECT が揃っているときだけ
// 行われる (無ければスキップされ、ビルドはそのまま通る)
export default withSentryConfig(withSerwist(withMDX(nextConfig)), {
  // アップロードしない通常のビルド (ローカル・CI) で警告ログを出さない
  silent: !process.env.SENTRY_AUTH_TOKEN,
  telemetry: false,
});
