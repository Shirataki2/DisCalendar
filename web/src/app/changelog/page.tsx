import type { Metadata } from "next";
import Content from "@/content/changelog.mdx";
import { ROUTES } from "@/lib/site";

// 更新履歴ページ。本文は src/content/changelog.mdx にあり、機能追加・変更のたびに追記する
// (書き方のルールは changelog.mdx 冒頭のコメント)

const TITLE = "更新履歴";
const DESCRIPTION =
  "DisCalendar の新機能や改善のお知らせ。Web (ブラウザ)・Bot (Discord)・API (サービス基盤) の更新内容をまとめています。";

export const metadata: Metadata = {
  title: TITLE,
  description: DESCRIPTION,
  openGraph: {
    title: TITLE,
    description: DESCRIPTION,
    url: ROUTES.changelog,
  },
};

export default function ChangelogPage() {
  return (
    <article>
      <header className="mb-8 border-b border-white/10 pb-6">
        <h1 className="text-3xl font-bold sm:text-4xl">{TITLE}</h1>
      </header>
      <Content />
    </article>
  );
}
