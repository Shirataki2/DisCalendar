import type { Metadata } from "next";
import Link from "next/link";
import { Logo } from "@/components/logo";
import { Tutorial } from "@/components/tutorial";
import { ROUTES } from "@/lib/site";

export const metadata: Metadata = {
  title: "操作を試す",
  description:
    "ログインもBotの導入も不要。練習用カレンダーで予定の作成・編集・通知・削除を体験できます。",
};

export default function TutorialPage() {
  return (
    <>
      <header className="border-b border-white/10">
        <div className="mx-auto flex max-w-[1600px] items-center justify-between gap-4 px-4 py-4 sm:px-8">
          <Link
            prefetch={false}
            href={ROUTES.home}
            aria-label="DisCalendar ホーム"
          >
            <Logo className="text-xl" />
          </Link>
          <Link
            prefetch={false}
            href={ROUTES.docs}
            className="text-sm text-muted-foreground underline underline-offset-4"
          >
            使い方を見る
          </Link>
        </div>
      </header>
      <main className="mx-auto flex w-full max-w-[1600px] flex-1 flex-col px-4 py-6 sm:px-8">
        <div className="mb-6 flex flex-wrap items-end justify-between gap-3 border-b border-white/10 pb-5">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">
              カレンダーを使ってみよう
            </h1>
            <p className="mt-2 text-sm text-muted-foreground">
              ログインもBotの導入も不要。まずはここで、予定をひとつ。
            </p>
          </div>
          <p className="rounded-full border border-indigo-400/30 bg-indigo-500/10 px-3 py-2 text-xs font-medium text-indigo-200">
            練習用・Discordには送信されません
          </p>
        </div>
        <Tutorial />
      </main>
    </>
  );
}
