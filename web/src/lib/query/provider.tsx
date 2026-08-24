"use client";

import {
  isServer,
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import type { ReactNode } from "react";
import { ApiError } from "@/lib/api/client";

function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        // SSR 直後のクライアントでの即時再取得を避ける
        staleTime: 30 * 1000,
        retry: (failureCount, error) => {
          // 認証・権限・入力エラーはリトライしても結果が変わらない
          if (error instanceof ApiError && error.status < 500) {
            return false;
          }
          return failureCount < 2;
        },
      },
    },
  });
}

let browserQueryClient: QueryClient | undefined;

function getQueryClient() {
  if (isServer) {
    // サーバーではリクエストごとに作る (リクエスト間でキャッシュを共有しない)
    return makeQueryClient();
  }
  // ブラウザでは Suspense による再レンダリングで作り直さないよう 1 つを使い回す
  browserQueryClient ??= makeQueryClient();
  return browserQueryClient;
}

export function QueryProvider({ children }: { children: ReactNode }) {
  const queryClient = getQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      {children}
      {/* devtools は開発ビルドだけで描画されるが、E2E (next dev) ではフローティングボタンが
          モバイル相当のビューポートでタップを遮るので出さない (#14) */}
      {!process.env.NEXT_PUBLIC_E2E && (
        <ReactQueryDevtools initialIsOpen={false} />
      )}
    </QueryClientProvider>
  );
}
