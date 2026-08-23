/// <reference lib="esnext" />
/// <reference lib="webworker" />
import type {
  PrecacheEntry,
  RuntimeCaching,
  SerwistGlobalConfig,
} from "serwist";
import { CacheFirst, ExpirationPlugin, Serwist } from "serwist";

// Service Worker の本体 (旧実装の @nuxtjs/pwa の workbox 設定に相当)。
// app/serwist/[path]/route.ts (createSerwistRoute) が esbuild でこのファイルを束ね、/serwist/sw.js として配信する。
// ブラウザ側の登録は components/service-worker-provider.tsx (dev では登録しない)。
//
// 方針:
// - precache (self.__SW_MANIFEST): ビルド時に .next/static/ (JS / CSS など) と public/ (アイコン) の一覧が埋め込まれる。
//   ハッシュ付きの資産なので、アプリシェルとして最初に取得して以後はキャッシュから返す
// - runtimeCaching: /_next/static/ 配下のうち precache に入らないもの (next/font が自己ホストするフォント
//   /_next/static/media/*.woff2 など。旧実装の「Google Fonts のキャッシュ」に相当) だけ CacheFirst で残す
// - ページ (HTML / RSC)・Better Auth (/api/auth/*)・Rust API (/local/api/*)・Discord CDN はどのルートにも
//   一致させない = Service Worker は関与せずブラウザがそのまま取りに行く。認証付きのレスポンスを
//   Cache Storage に残さないため、これらにはキャッシュ戦略を付けない

declare global {
  interface WorkerGlobalScope extends SerwistGlobalConfig {
    __SW_MANIFEST: (PrecacheEntry | string)[] | undefined;
  }
}

declare const self: ServiceWorkerGlobalScope;

const runtimeCaching: RuntimeCaching[] = [
  {
    // 内容が変わればファイル名のハッシュも変わるので、一度取れたものはそのまま使い回してよい
    matcher: ({ sameOrigin, url: { pathname } }) =>
      sameOrigin && pathname.startsWith("/_next/static/"),
    handler: new CacheFirst({
      cacheName: "next-static",
      plugins: [
        new ExpirationPlugin({
          maxEntries: 64,
          maxAgeSeconds: 30 * 24 * 60 * 60,
          maxAgeFrom: "last-used",
        }),
      ],
    }),
  },
];

const serwist = new Serwist({
  precacheEntries: self.__SW_MANIFEST,
  precacheOptions: {
    // 旧実装 (workbox) など別の形式の precache が残っていたら消す
    cleanupOutdatedCaches: true,
  },
  // デプロイ直後に古い SW が居座らないよう、新しい SW をすぐ有効にして開いているタブも引き取る
  skipWaiting: true,
  clientsClaim: true,
  runtimeCaching,
});

serwist.addEventListeners();
