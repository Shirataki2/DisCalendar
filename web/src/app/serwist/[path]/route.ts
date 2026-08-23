import { createSerwistRoute } from "@serwist/turbopack";

// Service Worker (/serwist/sw.js と /serwist/sw.js.map) を配信する Route Handler。
// @serwist/next は webpack プラグインなので Turbopack の Next 16 では使えず、代わりに @serwist/turbopack を使う。
// createSerwistRoute が src/app/sw.ts を esbuild で束ね、.next/static/ と public/ の一覧を precache マニフェストとして
// 埋め込む。force-static + generateStaticParams なので next build 時に生成され、本番 (standalone) の実行時に
// esbuild は要らない。dev では precache 無しで毎回ビルドされるが、登録自体を止めている
// (components/service-worker-provider.tsx) のでブラウザから取りに来ることはない
export const { dynamic, dynamicParams, revalidate, generateStaticParams, GET } =
  createSerwistRoute({
    swSrc: "src/app/sw.ts",
    // precache (アプリシェル) は JS / CSS だけにする。既定 (.next/static/ の画像類と public/ も含む) だと
    // LP / docs のスクリーンショットやアイコンまで初回に落としてしまう。画像やフォント (/_next/static/media/) は
    // 使われたときに sw.ts の runtimeCaching で残す
    globPatterns: [".next/static/**/*.{js,css}"],
    // 既定は esbuild-wasm (未インストール)。ネイティブの esbuild を使う
    useNativeEsbuild: true,
  });
