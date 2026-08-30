import { ExternalLinkIcon, HeartIcon } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { ROUTES, SUPPORT_SERVER_URL } from "@/lib/site";

// 支援 (ドネーション) のお願いページ。静的に生成される
// (Stripe Payment Links の URL は /donation/checkout が実行時に解決するので、ここには埋め込まない。/invite と同じ方式)

const DESCRIPTION =
  "DisCalendar は無料で利用できます。運営を続けるための任意の支援 (ドネーション) を受け付けています。";

export const metadata: Metadata = {
  title: "支援",
  description: DESCRIPTION,
  openGraph: {
    title: "支援",
    description: DESCRIPTION,
    url: ROUTES.donation,
  },
};

/** 注意書き。支援ボタンの下に並べる */
const NOTES = [
  "支援は任意です。支援の有無で利用できる機能は変わりません",
  "決済は Stripe 社の決済ページで行われます。カード情報が DisCalendar に伝わることはありません",
  "支援の性質上、決済完了後の返金はできません",
  "寄付金控除など税制上の優遇の対象ではありません",
] as const;

export default function DonationPage() {
  return (
    <>
      <SiteHeader />
      <main className="mx-auto w-full max-w-3xl flex-1 px-4 py-8 sm:px-6 lg:py-12">
        <article>
          <header className="mb-8 border-b border-white/10 pb-6">
            <h1 className="text-3xl font-bold sm:text-4xl">支援のお願い</h1>
          </header>
          <div className="space-y-6 leading-8 text-neutral-200">
            <p>
              DisCalendar
              は無料で利用でき、今後も無料で提供し続ける予定です。一方で、サーバー代やドメイン代などの運用費は運営者が負担しています。
            </p>
            <p>
              DisCalendar を気に入ってくださった方は、任意の支援 (ドネーション)
              で運営を応援していただけると励みになります。金額は決済ページで選べます。
            </p>
            <p className="pt-2">
              <a
                href={ROUTES.donationCheckout}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex h-12 w-full items-center justify-center gap-2 rounded-full bg-indigo-500 px-10 text-sm font-semibold tracking-wide text-white transition-colors hover:bg-indigo-400 sm:w-auto"
              >
                <HeartIcon className="size-4" aria-hidden />
                DisCalendar を支援する
                <ExternalLinkIcon className="size-4" aria-hidden />
              </a>
            </p>
            <ul className="list-disc space-y-1 pl-5 text-sm leading-7 text-neutral-400">
              {NOTES.map((note) => (
                <li key={note}>{note}</li>
              ))}
              <li>
                支援の決済に関する表記は
                <Link
                  href={ROUTES.tokushoho}
                  className="underline underline-offset-4 transition-colors hover:text-white"
                >
                  特定商取引法に基づく表記
                </Link>
                をご覧ください
              </li>
            </ul>
            <p className="border-t border-white/10 pt-6 text-neutral-300">
              金銭の支援でなくても、
              <a
                href={SUPPORT_SERVER_URL}
                target="_blank"
                rel="noopener noreferrer"
                className="underline underline-offset-4 transition-colors hover:text-white"
              >
                サポートサーバー
              </a>
              での不具合の報告や機能の提案も、DisCalendar
              にとって大きな支援になります。いつもご利用ありがとうございます。
            </p>
          </div>
        </article>
      </main>
      <SiteFooter />
    </>
  );
}
