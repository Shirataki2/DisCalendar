import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "カレンダー",
  // Discord の Activity (#350) 専用の画面なので検索に載せない
  robots: { index: false, follow: false },
};

// Discord の Activity の中で開く画面。iframe が狭いのでヘッダ・サイドバー・フッタは置かない。
// カレンダーは残りの高さを使う (flex-1 + min-h-0) ので、ダッシュボード (dashboard/layout.tsx) と同じく
// 高さを画面に固定する。min-h だけだと高さが決まらず、カレンダーの本体が 0 px になる
export default function ActivityLayout({ children }: LayoutProps<"/activity">) {
  return <div className="flex h-dvh flex-col">{children}</div>;
}
