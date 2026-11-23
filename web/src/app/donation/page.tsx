import { ExternalLinkIcon, HeartIcon } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { Message } from "@/components/language-provider";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { pageOpenGraph, ROUTES, SUPPORT_SERVER_URL } from "@/lib/site";

// 支援 (ドネーション) のお願いページ。静的に生成される
// (Stripe の決済ページ・カスタマーポータルの URL は /donation/checkout・/donation/monthly・/donation/portal が
// 実行時に解決するので、ここには埋め込まない。/invite と同じ方式。lib/donation.ts)

const DESCRIPTION =
  "DisCalendar は無料で利用できます。運営を続けるための任意の支援 (ドネーション) を受け付けています。";

export const metadata: Metadata = {
  title: "支援",
  description: DESCRIPTION,
  openGraph: pageOpenGraph({
    title: "支援",
    description: DESCRIPTION,
    url: ROUTES.donation,
  }),
};

/** 注意書き。支援ボタンの下に並べる */
const NOTES = [
  "支援は任意です。支援の有無で利用できる機能は変わりません",
  "決済は Stripe 社の決済ページで行われます。カード情報が DisCalendar に伝わることはありません",
  "毎月の支援は、解約するまで毎月自動で決済されます。解約はいつでもでき、次回の決済日の前日までに解約すると次回以降は請求されません",
  "支援の性質上、決済完了後の返金はできません (毎月の支援を解約しても、決済済みの月の分は返金されません)",
  "寄付金控除など税制上の優遇の対象ではありません",
] as const;

export default function DonationPage() {
  return (
    <>
      <SiteHeader />
      <main className="mx-auto w-full max-w-3xl flex-1 px-4 py-8 sm:px-6 lg:py-12">
        <article>
          <header className="mb-8 border-b border-white/10 pb-6">
            <h1 className="text-3xl font-bold sm:text-4xl">
              <Message message="支援のお願い" />
            </h1>
          </header>
          <div className="space-y-6 leading-8 text-neutral-200">
            <p>
              <Message message="DisCalendar は無料で利用でき、今後も無料で提供し続ける予定です。一方で、サーバー代やドメイン代などの運用費は運営者が負担しています。" />
            </p>
            <p>
              <Message message="DisCalendar を気に入ってくださった方は、任意の支援 (ドネーション) で運営を応援していただけると励みになります。運用費は毎月かかるため、少額でも毎月の支援は特に助かります。金額は決済ページで選べます。" />
            </p>
            <div className="flex flex-col gap-3 pt-2 sm:flex-row">
              <a
                href={ROUTES.donationMonthly}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex h-12 w-full items-center justify-center gap-2 rounded-full bg-indigo-500 px-10 text-sm font-semibold tracking-wide text-white transition-colors hover:bg-indigo-400 sm:w-auto"
              >
                <HeartIcon className="size-4" aria-hidden />
                <Message message="毎月支援する" />
                <ExternalLinkIcon className="size-4" aria-hidden />
              </a>
              <a
                href={ROUTES.donationCheckout}
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex h-12 w-full items-center justify-center gap-2 rounded-full border border-white/20 px-10 text-sm font-semibold tracking-wide text-white transition-colors hover:bg-white/10 sm:w-auto"
              >
                <Message message="1 回だけ支援する" />
                <ExternalLinkIcon className="size-4" aria-hidden />
              </a>
            </div>
            <ul className="list-disc space-y-1 pl-5 text-sm leading-7 text-neutral-400">
              {NOTES.map((note) => (
                <li key={note}>
                  <Message message={note} />
                </li>
              ))}
              <li>
                <Message message="支援の決済に関する表記は" />{" "}
                <Link
                  href={ROUTES.tokushoho}
                  className="underline underline-offset-4 transition-colors hover:text-white"
                >
                  <Message message="特定商取引法に基づく表記" />
                </Link>
                <Message message="をご覧ください" />
              </li>
            </ul>
            <section className="rounded-lg border border-white/10 p-5">
              <h2 className="text-base font-semibold text-white">
                <Message message="毎月支援している方へ" />
              </h2>
              <p className="mt-2 text-sm leading-7 text-neutral-300">
                <Message message="解約・支払い方法の変更・領収書の確認は、Stripe の支援の管理ページで行えます。決済時に入力したメールアドレスでログインしてください。" />
              </p>
              <p className="mt-3">
                <a
                  href={ROUTES.donationPortal}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 text-sm underline underline-offset-4 transition-colors hover:text-white"
                >
                  <Message message="支援の管理 (解約・カード変更)" />
                  <ExternalLinkIcon className="size-3.5" aria-hidden />
                </a>
              </p>
            </section>
            <p className="border-t border-white/10 pt-6 text-neutral-300">
              <Message message="不具合の報告や機能の提案も、大きな支援になります。" />{" "}
              <a
                href={SUPPORT_SERVER_URL}
                target="_blank"
                rel="noopener noreferrer"
                className="underline underline-offset-4 transition-colors hover:text-white"
              >
                <Message message="サポートサーバー" />
              </a>{" "}
              <Message message="へお気軽にお寄せください。いつもご利用ありがとうございます。" />
            </p>
          </div>
        </article>
      </main>
      <SiteFooter />
    </>
  );
}
