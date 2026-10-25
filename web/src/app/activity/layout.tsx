import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "カレンダー",
  // Discord の Activity (#350) 専用の画面なので検索に載せない
  robots: { index: false, follow: false },
};

// Discord の Activity の中で開く画面。iframe が狭いのでヘッダ・サイドバー・フッタは置かない
export default function ActivityLayout({ children }: LayoutProps<"/activity">) {
  return <div className="flex min-h-svh flex-1 flex-col">{children}</div>;
}
