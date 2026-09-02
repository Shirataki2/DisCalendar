import type { Metadata } from "next";
import Link from "next/link";
import { ERROR_SCREEN_ACTION, ErrorScreen } from "@/components/error-screen";
import { ROUTES } from "@/lib/site";

// 存在しない URL と、ページが notFound() を投げたとき (使い方・規約に無いページ、ダッシュボードの不正なサーバー ID、
// 管理者以外の /admin など) に出す 404 画面 (#61)。app/ 直下の not-found は入れ子のレイアウト (docs のサイドバーなど) を
// 伴わずに root layout の直下に描画されるので、ヘッダ・フッタは ErrorScreen が付ける。
// Next.js が 404 として返すページには noindex も自動で付く

export const metadata: Metadata = {
  title: "ページが見つかりません",
};

export default function NotFound() {
  return (
    <ErrorScreen
      code="404"
      title="ページが見つかりません"
      actions={
        <>
          <Link href={ROUTES.home} className={ERROR_SCREEN_ACTION.primary}>
            トップページへ
          </Link>
          <Link
            href={ROUTES.dashboard}
            className={ERROR_SCREEN_ACTION.secondary}
          >
            サーバー一覧へ
          </Link>
        </>
      }
    >
      <p>
        お探しのページは存在しないか、移動または削除された可能性があります。URL
        に間違いがないかお確かめください。
      </p>
    </ErrorScreen>
  );
}
