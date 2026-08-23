"use client";

import { SerwistProvider } from "@serwist/turbopack/react";
import { type ReactNode, useEffect } from "react";

// dev では Service Worker を登録しない (Turbopack の HMR やページの再コンパイルの邪魔になるため)
const DISABLED = process.env.NODE_ENV === "development";

/**
 * Service Worker (/serwist/sw.js、本体は app/sw.ts) の登録。
 * 本番ビルドでだけ登録し、dev では登録しないうえ、残っている登録とキャッシュも消す
 * (同じオリジンで本番ビルドを動かした後に dev を起動すると、その SW が dev のページも制御してしまう)。
 */
export function ServiceWorkerProvider({ children }: { children: ReactNode }) {
  useEffect(() => {
    if (!DISABLED || !("serviceWorker" in navigator)) return;
    navigator.serviceWorker.getRegistrations().then((registrations) => {
      for (const registration of registrations) {
        registration.unregister();
      }
    });
    caches.keys().then((keys) => {
      for (const key of keys) {
        caches.delete(key);
      }
    });
  }, []);

  return (
    <SerwistProvider
      swUrl="/serwist/sw.js"
      disable={DISABLED}
      // ページ遷移のたびに遷移先を SW にキャッシュさせる機能。ページは SW でキャッシュしない方針なので切る
      // (history.pushState を差し替える実装でもあるので、Next のルーターに触らせない)
      cacheOnNavigation={false}
      // オンライン復帰時の自動リロード。入力中の予定ダイアログが消えるので切る
      reloadOnOnline={false}
    >
      {children}
    </SerwistProvider>
  );
}
