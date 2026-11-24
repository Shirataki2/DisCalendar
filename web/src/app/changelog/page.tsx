import type { Metadata } from "next";
import Link from "next/link";
import Content from "@/content/changelog.mdx";
import { pageOpenGraph, ROUTES } from "@/lib/site";

// 更新履歴ページ。本文はリリース準備時に changelog.d から src/content/changelog.mdx へ集約する。

const TITLE = "更新履歴";
const DESCRIPTION =
  "DisCalendar の新機能や改善のお知らせ。Web (ブラウザ)・Bot (Discord)・API (サービス基盤) の更新内容をまとめています。";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  openGraph: pageOpenGraph({
    title: TITLE,
    description: DESCRIPTION,
    url: ROUTES.changelog,
  }),
};

export default function ChangelogPage() {
  return (
    <article>
      <header className="mb-8 border-b border-white/10 pb-6">
        <h1 className="text-3xl font-bold sm:text-4xl">{TITLE}</h1>
        {/* 支援ページへの控えめな案内 (#371)。見出しの下に 1 行だけ置き、本文の邪魔をしない */}
        <p className="mt-3 text-sm text-neutral-400">
          DisCalendar は無料で使えます。気に入っていただけたら、
          <Link
            href={ROUTES.donation}
            className="font-medium text-neutral-200 underline underline-offset-4"
          >
            支援ページ
          </Link>
          から運営を応援していただけると励みになります (任意です)。
        </p>
      </header>
      <Content />
    </article>
  );
}
