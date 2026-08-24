import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";

// 更新履歴ページのレイアウト。規約ページ (app/support) と同じく、
// LP と同じヘッダ・フッタで本文だけを出す
export default function ChangelogLayout({
  children,
}: LayoutProps<"/changelog">) {
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
