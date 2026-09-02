import type { ReactNode } from "react";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { SUPPORT_SERVER_URL } from "@/lib/site";

/**
 * 404 / エラー画面 (app/not-found.tsx, app/error.tsx) の共通レイアウト (#61)。
 * 旧実装の layouts/error.vue (ステータスコードを大きく出し、その下に案内文) に相当する。
 * LP・規約ページと同じヘッダ・フッタで挟み、本文は中央に寄せる。
 *
 * これらの画面はどの URL でも出るので、ダッシュボード配下 (/dashboard/...) でライトテーマを選んでいる利用者にも表示される。
 * ヘッダ・フッタとこの画面の配色はダーク前提 (components/theme-provider.tsx の方針どおり、ライトはダッシュボードだけ) なので、
 * 全体を .dark で包んで配色トークンをダークに固定する (<html> の class はページによらず next-themes が決めるため、ここで上書きする)
 */
export function ErrorScreen({
  code,
  title,
  children,
  actions,
  footnote,
}: {
  /** 見出しの上に大きく出すステータスコード (404 など)。分からないときは省く */
  code?: string;
  title: string;
  /** 案内文 */
  children: ReactNode;
  /** 移動先のリンクやボタン。ERROR_SCREEN_ACTION の class で見た目を揃える */
  actions: ReactNode;
  /** サポートサーバーの案内の前に出す補足 (エラー ID など) */
  footnote?: ReactNode;
}) {
  return (
    <div className="dark flex flex-1 flex-col bg-background text-foreground">
      <SiteHeader />
      <main className="flex flex-1 flex-col items-center justify-center gap-6 px-4 py-16 text-center sm:px-6">
        {code ? (
          <p
            aria-hidden
            className="font-thin text-7xl tracking-[0.2em] text-neutral-500 sm:text-8xl"
          >
            {code}
          </p>
        ) : null}
        <h1 className="text-2xl font-bold sm:text-3xl">{title}</h1>
        <div className="max-w-md text-sm leading-7 text-neutral-300">
          {children}
        </div>
        <div className="flex flex-wrap items-center justify-center gap-3 pt-2">
          {actions}
        </div>
        <div className="space-y-1 text-xs leading-6 text-neutral-400">
          {footnote}
          <p>
            お困りのときは
            <a
              href={SUPPORT_SERVER_URL}
              target="_blank"
              rel="noopener noreferrer"
              className="mx-1 underline underline-offset-4 transition-colors hover:text-white"
            >
              サポートサーバー
            </a>
            へご連絡ください
          </p>
        </div>
      </main>
      <SiteFooter />
    </div>
  );
}

/** ErrorScreen の actions に置くリンク・ボタンの見た目 (LP のボタンと同じ丸いピル型) */
export const ERROR_SCREEN_ACTION = {
  primary:
    "inline-flex h-11 items-center justify-center rounded-full bg-indigo-500 px-8 text-sm font-semibold tracking-wide text-white transition-colors hover:bg-indigo-400",
  secondary:
    "inline-flex h-11 items-center justify-center rounded-full border border-white/20 px-8 text-sm font-semibold tracking-wide transition-colors hover:bg-white/10",
} as const;
