"use client";

import { CheckIcon, ExternalLinkIcon, TriangleAlertIcon } from "lucide-react";
import Image from "next/image";
import Link from "next/link";
import { useLanguage } from "@/components/language-provider";
import { inviteHref, ROUTES } from "@/lib/site";
import {
  findUsecasePage,
  solutionDocPath,
  USECASE_COMMON_SOLUTIONS,
  USECASE_HERO_IMAGE,
  USECASE_PAGES,
  type UsecaseSolution,
  usecasePath,
} from "@/lib/usecases";

// 用途別の紹介ページの本文。構成は「よくある困りごと → DisCalendar での解決 → 導入ボタン」。
// 文章は LP と同じく t() で英訳を引く (lib/i18n/messages.ts)

const buttonBase =
  "inline-flex h-12 w-full items-center justify-center gap-2 rounded-full px-7 text-sm font-semibold tracking-wide transition-colors sm:w-auto";

export function UsecaseContent({ slug }: { slug: string }) {
  const { t, language } = useLanguage();
  const page = findUsecasePage(slug);
  if (!page) return null;
  const others = USECASE_PAGES.filter((other) => other.slug !== page.slug);

  const inviteButton = (
    <a
      href={inviteHref(page.inviteSource)}
      target="_blank"
      rel="noopener noreferrer"
      className={`${buttonBase} bg-indigo-500 text-white hover:bg-indigo-400 sm:px-12`}
    >
      {t("BOT を導入する")}
      <ExternalLinkIcon className="size-4" aria-hidden />
    </a>
  );

  return (
    <main lang={language} className="flex-1">
      <section className="relative overflow-hidden">
        <div
          aria-hidden
          className="pointer-events-none absolute inset-x-0 top-0 -z-10 h-[28rem] bg-[radial-gradient(ellipse_at_top,rgba(88,101,242,0.28),transparent_65%)]"
        />
        <div className="mx-auto flex max-w-4xl flex-col items-center px-4 pt-16 pb-12 text-center sm:px-6 sm:pt-20">
          <p className="mb-4 text-sm font-medium tracking-widest text-indigo-300">
            {t("{name}で使う", { name: t(page.name) })}
          </p>
          <h1 className="text-3xl leading-tight font-bold sm:text-5xl sm:leading-tight">
            {t(page.headline)}
          </h1>
          <p className="mt-6 max-w-2xl text-base leading-8 text-neutral-300">
            {t(page.lead)}
          </p>
          <div className="mt-8 flex w-full flex-col items-center gap-3 sm:w-auto sm:flex-row">
            {inviteButton}
            <Link
              href={ROUTES.tutorial}
              className={`${buttonBase} border border-indigo-300/40 text-indigo-100 hover:bg-indigo-500/10`}
            >
              {t("ログインせずに操作を試す")}
            </Link>
          </div>
        </div>
        <div className="mx-auto max-w-5xl px-4 pb-16 sm:px-6">
          <figure className="overflow-hidden rounded-xl border border-white/10 bg-surface shadow-2xl shadow-black/50">
            <Image
              src={USECASE_HERO_IMAGE.src}
              alt={t(USECASE_HERO_IMAGE.alt)}
              priority
              sizes="(min-width: 1024px) 1024px, 100vw"
              className="h-auto w-full"
            />
          </figure>
        </div>
      </section>

      <section
        aria-labelledby="problems-heading"
        className="border-t border-white/10 bg-surface/40"
      >
        <div className="mx-auto max-w-4xl px-4 py-16 sm:px-6">
          <h2
            id="problems-heading"
            className="text-center text-2xl font-bold sm:text-3xl"
          >
            {t("よくある困りごと")}
          </h2>
          <ul className="mt-10 grid gap-4">
            {page.problems.map((problem) => (
              <li
                key={problem}
                className="flex gap-3 rounded-xl border border-white/10 bg-background p-5 text-sm leading-7 text-neutral-200 sm:text-base"
              >
                <TriangleAlertIcon
                  className="mt-1 size-5 shrink-0 text-amber-300"
                  aria-hidden
                />
                {t(problem)}
              </li>
            ))}
          </ul>
        </div>
      </section>

      <section
        aria-labelledby="solutions-heading"
        className="border-t border-white/10"
      >
        <div className="mx-auto max-w-5xl px-4 py-16 sm:px-6 sm:py-20">
          <h2
            id="solutions-heading"
            className="text-center text-2xl font-bold sm:text-3xl"
          >
            {t("DisCalendar なら")}
          </h2>
          <div className="mt-12 grid items-start gap-10 md:grid-cols-2">
            <ul className="grid gap-8">
              {page.solutions.map((solution) => (
                <Solution key={solution.title} solution={solution} />
              ))}
            </ul>
            <figure className="overflow-hidden rounded-xl border border-white/10 shadow-xl shadow-black/40 md:sticky md:top-24">
              <Image
                src={page.image.src}
                alt={t(page.image.alt)}
                sizes="(min-width: 1024px) 480px, (min-width: 768px) 50vw, 100vw"
                className="h-auto w-full"
              />
            </figure>
          </div>
          <h3 className="mt-16 text-lg font-semibold">
            {t("ほかにもこんなことができます")}
          </h3>
          <ul className="mt-6 grid gap-6 sm:grid-cols-2">
            {USECASE_COMMON_SOLUTIONS.map((solution) => (
              <Solution key={solution.title} solution={solution} card />
            ))}
          </ul>
        </div>
      </section>

      <section className="border-t border-white/10 bg-[radial-gradient(ellipse_at_bottom,rgba(88,101,242,0.24),transparent_70%)]">
        <div className="mx-auto flex max-w-4xl flex-col items-center gap-6 px-4 py-16 text-center sm:px-6 sm:py-20">
          <h2 className="text-2xl font-bold sm:text-3xl">
            {t("まずは Bot をサーバーに追加してみてください")}
          </h2>
          <p className="max-w-xl text-sm leading-7 text-neutral-300">
            {t(
              "無料で使えます。Bot を追加したら、Web にログインして通知先チャンネルを選ぶだけで使い始められます。",
            )}
          </p>
          <div className="flex w-full flex-col gap-3 sm:w-auto sm:flex-row">
            {inviteButton}
            <Link
              href={ROUTES.docs}
              className={`${buttonBase} border border-white/20 text-neutral-100 hover:bg-white/10`}
            >
              {t("使い方を見る")}
            </Link>
          </div>
          {others.length > 0 && (
            <p className="mt-4 text-sm text-neutral-400">
              {t("ほかの使い方:")}{" "}
              {others.map((other, index) => (
                <span key={other.slug}>
                  {index > 0 && " / "}
                  <Link
                    href={usecasePath(other.slug)}
                    className="underline underline-offset-4 hover:text-white"
                  >
                    {t(other.name)}
                  </Link>
                </span>
              ))}
            </p>
          )}
        </div>
      </section>
    </main>
  );
}

function Solution({
  solution,
  card = false,
}: {
  solution: UsecaseSolution;
  card?: boolean;
}) {
  const { t } = useLanguage();
  // 共通の機能は「ほかにもこんなことができます」(h3) の下なので 1 段下げる
  const Heading = card ? "h4" : "h3";
  return (
    <li
      className={
        card
          ? "flex flex-col gap-2 rounded-xl border border-white/10 bg-surface/40 p-6"
          : "flex gap-4"
      }
    >
      {!card && (
        <span className="flex size-8 shrink-0 items-center justify-center rounded-full bg-indigo-500/15 text-indigo-300">
          <CheckIcon className="size-4" aria-hidden />
        </span>
      )}
      <div className="flex flex-col gap-2">
        <Heading className="font-semibold sm:text-lg">
          {t(solution.title)}
        </Heading>
        <p className="text-sm leading-7 text-neutral-300">{t(solution.body)}</p>
        <Link
          href={solutionDocPath(solution)}
          className="text-sm text-indigo-300 underline underline-offset-4 hover:text-indigo-200"
        >
          {t("くわしい使い方")}
        </Link>
      </div>
    </li>
  );
}
