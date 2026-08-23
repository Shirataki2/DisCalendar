import { DocsNav } from "@/components/docs/docs-nav";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";

// docs (使い方ページ) 共通のレイアウト。LP と同じヘッダ・フッタに、
// PC では左にページ一覧のサイドバー、スマホでは本文の上に横並びの目次を出す
export default function DocsLayout({ children }: LayoutProps<"/docs">) {
  return (
    <>
      <SiteHeader />
      <div className="mx-auto flex w-full max-w-6xl flex-1 gap-10 px-4 py-8 sm:px-6 lg:py-12">
        <aside className="hidden w-56 shrink-0 lg:block">
          <div className="sticky top-24">
            <p className="mb-3 px-3 text-xs font-semibold tracking-widest text-neutral-500 uppercase">
              使い方
            </p>
            <DocsNav orientation="vertical" />
          </div>
        </aside>
        <div className="min-w-0 flex-1">
          <div className="mb-6 lg:hidden">
            <DocsNav orientation="horizontal" />
          </div>
          {children}
        </div>
      </div>
      <SiteFooter />
    </>
  );
}
