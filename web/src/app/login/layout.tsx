import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";

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
