import {
  BellRingIcon,
  CalendarDaysIcon,
  ExternalLinkIcon,
  ShieldCheckIcon,
  SlashSquareIcon,
} from "lucide-react";
import Image from "next/image";
import Link from "next/link";
import type { ReactNode } from "react";
import calendarShot from "@/assets/lp/calendar.png";
import dialogShot from "@/assets/lp/dialog.png";
import settingsShot from "@/assets/lp/settings.png";
import { Logo } from "@/components/logo";
import { SessionLink } from "@/components/session-link";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { ROUTES, SUPPORT_SERVER_URL } from "@/lib/site";

// LP (トップページ)。静的生成される (セッションの有無は SessionLink がブラウザ側で判定し、
// Bot の招待 URL は /invite が実行時に解決する)

const buttonBase =
  "inline-flex h-12 w-full items-center justify-center gap-2 rounded-full px-7 text-sm font-semibold tracking-wide transition-colors sm:w-auto";

export default function Home() {
  return (
    <>
      <SiteHeader />
      <main className="flex-1">
        <Hero />
        <Features />
        <Steps />
        <BottomCta />
      </main>
      <SiteFooter />
    </>
  );
}

function Hero() {
  return (
    <section className="relative overflow-hidden">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 top-0 -z-10 h-[32rem] bg-[radial-gradient(ellipse_at_top,rgba(88,101,242,0.32),transparent_65%)]"
      />
      <div className="mx-auto flex max-w-6xl flex-col items-center px-4 pt-16 pb-12 text-center sm:px-6 sm:pt-24">
        <p className="mb-4 text-sm font-medium tracking-widest text-indigo-300">
          Discord 用予定管理 Bot
        </p>
        <h1>
          <Logo className="text-5xl sm:text-7xl" />
        </h1>
        <p className="mt-8 max-w-2xl text-base leading-8 text-neutral-300 sm:text-lg">
          DisCalendarはDiscord用のカレンダーアプリです。予定の作成から通知まで面倒なコマンド操作はほとんど必要ありません。
          使い慣れたブラウザから、どこでも予定の追加や編集をすることができます。
        </p>
        <div className="mt-10 flex w-full flex-col items-center gap-3 sm:w-auto">
          <a
            href={ROUTES.invite}
            target="_blank"
            rel="noopener noreferrer"
            className={`${buttonBase} bg-indigo-500 text-white hover:bg-indigo-400 sm:px-12`}
          >
            BOT を導入する
            <ExternalLinkIcon className="size-4" aria-hidden />
          </a>
          <p className="my-1 flex w-full items-center gap-4 text-xs tracking-widest text-neutral-500 uppercase before:h-px before:flex-1 before:bg-white/15 after:h-px after:flex-1 after:bg-white/15">
            OR 既に導入済みの方は
          </p>
          <div className="flex w-full flex-col gap-3 sm:flex-row sm:justify-center">
            <SessionLink
              className={`${buttonBase} bg-[#5865F2] text-white hover:bg-[#4752c4]`}
            />
            <a
              href={SUPPORT_SERVER_URL}
              target="_blank"
              rel="noopener noreferrer"
              className={`${buttonBase} border border-white/20 text-neutral-100 hover:bg-white/10`}
            >
              サポートサーバーへ参加
            </a>
            <Link
              href={ROUTES.docs}
              className={`${buttonBase} border border-white/20 text-neutral-100 hover:bg-white/10`}
            >
              使い方を見る
            </Link>
          </div>
        </div>
      </div>
      <div className="mx-auto max-w-6xl px-4 pb-16 sm:px-6">
        <figure className="overflow-hidden rounded-xl border border-white/10 bg-surface shadow-2xl shadow-black/50">
          <Image
            src={calendarShot}
            alt="DisCalendar のカレンダー画面。月表示に色分けされた予定が並んでいる"
            priority
            sizes="(min-width: 1152px) 1152px, 100vw"
            className="h-auto w-full"
          />
        </figure>
      </div>
    </section>
  );
}

interface Showcase {
  icon: ReactNode;
  title: string;
  body: string;
  image: { src: typeof dialogShot; alt: string };
}

/** スクリーンショット付きで見せる機能 (画像と文章を左右に並べる) */
const SHOWCASES: Showcase[] = [
  {
    icon: <CalendarDaysIcon className="size-6" aria-hidden />,
    title: "ブラウザから予定を追加・編集",
    body: "Discord アカウントでログインすると、参加しているサーバーのカレンダーを開けます。日付をクリックして予定を作り、ドラッグで移動。月・週・日の表示を切り替えて、スマホからも操作できます。",
    image: {
      src: dialogShot,
      alt: "予定の編集ダイアログ。タイトル・日時・色・通知・説明を入力できる",
    },
  },
  {
    icon: <ShieldCheckIcon className="size-6" aria-hidden />,
    title: "編集できる人を制限 (restricted モード)",
    body: "サーバー設定で restricted モードを有効にすると、「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限を持つメンバーだけが予定を追加・編集・削除できます。閲覧はメンバー全員ができます。",
    image: {
      src: settingsShot,
      alt: "サーバー設定ダイアログ。予定の編集を管理権限を持つユーザーに限定するチェックボックス",
    },
  },
];

interface Feature {
  icon: ReactNode;
  title: string;
  body: string;
}

const FEATURES: Feature[] = [
  {
    icon: <BellRingIcon className="size-6" aria-hidden />,
    title: "Discord へ自動で通知",
    body: "予定ごとに「30 分前」「1 日前」のような事前通知を最大 10 件まで設定できます。時刻になると Bot が /init で決めたチャンネルに埋め込みメッセージを投稿するので、リマインドの手間がありません。",
  },
  {
    icon: <SlashSquareIcon className="size-6" aria-hidden />,
    title: "Discord からも操作できる",
    body: "/create で予定の作成、/list で一覧表示、/init で通知先チャンネルの設定。ブラウザを開かなくても Discord のスラッシュコマンドから同じカレンダーを扱えます。",
  },
];

function FeatureIcon({ children }: { children: ReactNode }) {
  return (
    <span className="flex size-11 items-center justify-center rounded-lg bg-indigo-500/15 text-indigo-300">
      {children}
    </span>
  );
}

function Features() {
  return (
    <section
      aria-labelledby="features-heading"
      className="border-t border-white/10 bg-surface/40"
    >
      <div className="mx-auto max-w-6xl px-4 py-16 sm:px-6 sm:py-24">
        <h2
          id="features-heading"
          className="text-center text-2xl font-bold sm:text-3xl"
        >
          できること
        </h2>
        <p className="mx-auto mt-3 max-w-xl text-center text-sm leading-7 text-neutral-400">
          サーバーの予定をひとつのカレンダーにまとめて、通知まで Bot
          に任せられます。
        </p>
        <div className="mt-12 flex flex-col gap-16">
          {SHOWCASES.map((item, index) => (
            <div
              key={item.title}
              className="grid items-center gap-8 md:grid-cols-2 md:gap-12"
            >
              <div
                className={`flex flex-col gap-4 ${index % 2 === 1 ? "md:order-2" : ""}`}
              >
                <FeatureIcon>{item.icon}</FeatureIcon>
                <h3 className="text-xl font-semibold sm:text-2xl">
                  {item.title}
                </h3>
                <p className="text-sm leading-7 text-neutral-300 sm:text-base sm:leading-8">
                  {item.body}
                </p>
              </div>
              <figure className="overflow-hidden rounded-xl border border-white/10 shadow-xl shadow-black/40">
                <Image
                  src={item.image.src}
                  alt={item.image.alt}
                  sizes="(min-width: 1152px) 552px, (min-width: 768px) 50vw, 100vw"
                  className="h-auto w-full"
                />
              </figure>
            </div>
          ))}
          <ul className="grid gap-6 sm:grid-cols-2">
            {FEATURES.map((feature) => (
              <li
                key={feature.title}
                className="flex flex-col gap-3 rounded-xl border border-white/10 bg-background p-6"
              >
                <FeatureIcon>{feature.icon}</FeatureIcon>
                <h3 className="text-lg font-semibold">{feature.title}</h3>
                <p className="text-sm leading-7 text-neutral-300">
                  {feature.body}
                </p>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}

const STEPS = [
  {
    title: "Bot をサーバーに招待する",
    body: "「BOT を導入する」から Discord の画面でサーバーを選んで追加します。サーバーの管理権限が必要です。",
  },
  {
    title: "/init で通知先を決める",
    body: "通知を流したいチャンネルで /init を実行すると、そのチャンネルに予定の事前通知が届くようになります。",
  },
  {
    title: "ログインして予定を追加する",
    body: "Discord アカウントでログインし、サーバーを選んでカレンダーを開けば、あとは日付をクリックするだけです。",
  },
] as const;

function Steps() {
  return (
    <section
      aria-labelledby="steps-heading"
      className="border-t border-white/10"
    >
      <div className="mx-auto max-w-6xl px-4 py-16 sm:px-6 sm:py-24">
        <h2
          id="steps-heading"
          className="text-center text-2xl font-bold sm:text-3xl"
        >
          はじめかた
        </h2>
        <ol className="mt-12 grid gap-8 sm:grid-cols-3">
          {STEPS.map((step, index) => (
            <li key={step.title} className="flex gap-4">
              <span className="flex size-9 shrink-0 items-center justify-center rounded-full bg-indigo-500 text-sm font-bold text-white">
                {index + 1}
              </span>
              <div>
                <h3 className="font-semibold">{step.title}</h3>
                <p className="mt-2 text-sm leading-7 text-neutral-300">
                  {step.body}
                </p>
              </div>
            </li>
          ))}
        </ol>
        <p className="mt-10 text-center text-sm text-neutral-400">
          くわしい手順は
          <Link
            href={ROUTES.docs}
            className="mx-1 underline underline-offset-4 hover:text-white"
          >
            使い方
          </Link>
          を参照してください。
        </p>
      </div>
    </section>
  );
}

function BottomCta() {
  return (
    <section className="border-t border-white/10 bg-[radial-gradient(ellipse_at_bottom,rgba(88,101,242,0.24),transparent_70%)]">
      <div className="mx-auto flex max-w-6xl flex-col items-center gap-6 px-4 py-16 text-center sm:px-6 sm:py-24">
        <h2 className="text-2xl font-bold sm:text-3xl">
          まずは Bot をサーバーに追加してみてください
        </h2>
        <p className="max-w-xl text-sm leading-7 text-neutral-300">
          無料で使えます。困ったときはサポートサーバーで質問してください。
        </p>
        <div className="flex w-full flex-col gap-3 sm:w-auto sm:flex-row">
          <a
            href={ROUTES.invite}
            target="_blank"
            rel="noopener noreferrer"
            className={`${buttonBase} bg-indigo-500 text-white hover:bg-indigo-400 sm:px-12`}
          >
            BOT を導入する
            <ExternalLinkIcon className="size-4" aria-hidden />
          </a>
          <SessionLink
            className={`${buttonBase} border border-white/20 text-neutral-100 hover:bg-white/10`}
          />
        </div>
      </div>
    </section>
  );
}
