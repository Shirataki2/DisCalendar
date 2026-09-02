"use client";

// ページやレイアウトの描画中に起きた未捕捉の例外を受け止めるエラー画面 (#61。App Router の error)。
// root layout 自体の失敗は global-error.tsx が受ける。
// エラー境界はクライアントコンポーネントでなければならないので、エラーは global-error と同じく
// ここから Sentry へ送る (#17。DSN 未設定なら何もしない)
import * as Sentry from "@sentry/nextjs";
import Link from "next/link";
import { useEffect } from "react";
import { ERROR_SCREEN_ACTION, ErrorScreen } from "@/components/error-screen";
import { ROUTES } from "@/lib/site";

export default function ErrorPage({
  error,
  retry,
}: {
  error: Error & { digest?: string };
  /** 境界の中身を取得し直して描画し直す (成功すればこの画面は消える) */
  retry: () => void;
}) {
  useEffect(() => {
    Sentry.captureException(error);
  }, [error]);

  return (
    <ErrorScreen
      title="エラーが発生しました"
      actions={
        <>
          <button
            type="button"
            onClick={() => retry()}
            className={ERROR_SCREEN_ACTION.primary}
          >
            再試行
          </button>
          <Link href={ROUTES.home} className={ERROR_SCREEN_ACTION.secondary}>
            トップページへ
          </Link>
        </>
      }
      footnote={
        // サーバー側で起きたエラーには digest (サーバーのログや Sentry と突き合わせる ID) が付く。
        // 問い合わせに添えてもらえるように見せておく
        error.digest ? (
          <p>
            エラー ID: <span className="font-mono">{error.digest}</span>
          </p>
        ) : null
      }
    >
      <p>
        ページの表示中に問題が発生しました。「再試行」を押しても直らないときは、一度ログアウトしてからもう一度アクセスしてみてください。
      </p>
    </ErrorScreen>
  );
}
