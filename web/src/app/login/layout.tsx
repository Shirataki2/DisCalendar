import type { Metadata } from "next";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";

// ログイン画面は検索結果に出しても役に立たないので載せない (リンク先はたどってよい)
export const metadata: Metadata = {
  title: "ログイン",
  robots: { index: false, follow: true },
};

// ログイン画面のレイアウト。LP や規約ページと同じヘッダ・フッタで挟み、
// ログインせずに使い方や規約などへ移動できるようにする
export default function LoginLayout({ children }: LayoutProps<"/login">) {
  return (
    <>
      <SiteHeader />
      {children}
      <SiteFooter />
    </>
  );
}
