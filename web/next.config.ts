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

export default nextConfig;
