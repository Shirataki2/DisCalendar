import type { NextConfig } from "next";

// Rust API (api/) の URL。rewrites はサーバー側で解決されるので、この値がブラウザに出ることはない
const API_URL = process.env.API_URL ?? "http://127.0.0.1:8080";

const nextConfig: NextConfig = {
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
