"use client";

// root layout ごと描画に失敗したときの最後の受け皿 (App Router の global-error)。
// root layout を置き換えるので html/body を自前で描く必要があり、globals.css も効かない前提で
// インラインスタイルだけにしてある。エラーは Sentry へ送る (#17。DSN 未設定なら何もしない)
import * as Sentry from "@sentry/nextjs";
import { useEffect } from "react";

export default function GlobalError({
  error,
  retry,
}: {
  error: Error & { digest?: string };
  retry: () => void;
}) {
  useEffect(() => {
    Sentry.captureException(error);
  }, [error]);

  return (
    <html lang="ja">
      <body
        style={{
          margin: 0,
          minHeight: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontFamily: "system-ui, sans-serif",
        }}
      >
        <div style={{ textAlign: "center", padding: "2rem" }}>
          <h1 style={{ fontSize: "1.25rem" }}>エラーが発生しました</h1>
          <p style={{ color: "#666" }}>
            ページの表示に失敗しました。時間をおいて再度お試しください。
          </p>
          <button
            type="button"
            onClick={() => retry()}
            style={{
              marginTop: "1rem",
              padding: "0.5rem 1.5rem",
              borderRadius: "0.5rem",
              border: "none",
              background: "#5865F2",
              color: "#fff",
              cursor: "pointer",
            }}
          >
            再読み込み
          </button>
        </div>
      </body>
    </html>
  );
}
