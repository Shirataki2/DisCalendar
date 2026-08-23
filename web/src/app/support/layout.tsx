import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";

// 規約ページ (利用規約 / プライバシーポリシー) 共通のレイアウト。
// docs のようなサイドナビは持たず、LP と同じヘッダ・フッタで本文だけを出す
export default function SupportLayout({ children }: LayoutProps<"/support">) {
  return (
    <>
      <SiteHeader />
      <div className="mx-auto w-full max-w-3xl flex-1 px-4 py-8 sm:px-6 lg:py-12">
        {children}
      </div>
      <SiteFooter />
    </>
  );
}
