import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import {
  findSupportPage,
  formatUpdatedAt,
  SUPPORT_PAGES,
  supportPath,
} from "@/lib/support";

// 規約ページ。本文は src/content/support/<slug>.mdx にあり、SUPPORT_PAGES の分だけビルド時に静的生成する
// (それ以外の slug は dynamicParams = false で 404)。URL は旧実装 (@nuxt/content) の /support/<slug> のまま

export const dynamicParams = false;

export function generateStaticParams() {
  return SUPPORT_PAGES.map((page) => ({ slug: page.slug }));
}

export async function generateMetadata({
  params,
}: PageProps<"/support/[slug]">): Promise<Metadata> {
  const { slug } = await params;
  const page = findSupportPage(slug);
  if (!page) return {};
  return {
    title: page.title,
    description: page.description,
    openGraph: {
      title: page.title,
      description: page.description,
      url: supportPath(page.slug),
    },
  };
}

export default async function SupportPage({
  params,
}: PageProps<"/support/[slug]">) {
  const { slug } = await params;
  const page = findSupportPage(slug);
  if (!page) notFound();

  // slug は SUPPORT_PAGES の値に限られる (dynamicParams = false) ので、動的 import の対象も content/support 配下に閉じる
  const { default: Content } = await import(`@/content/support/${slug}.mdx`);

  return (
    <article>
      <header className="mb-8 border-b border-white/10 pb-6">
        <h1 className="text-3xl font-bold sm:text-4xl">{page.title}</h1>
        <p className="mt-3 text-sm text-neutral-400">
          最終更新日: {formatUpdatedAt(page.updatedAt)}
        </p>
      </header>
      <Content />
      <nav className="mt-12 border-t border-white/10 pt-6 text-sm">
        <ul className="flex flex-wrap gap-x-5 gap-y-2 text-neutral-400">
          {SUPPORT_PAGES.filter((other) => other.slug !== page.slug).map(
            (other) => (
              <li key={other.slug}>
                <Link
                  href={supportPath(other.slug)}
                  className="transition-colors hover:text-white"
                >
                  {other.title}
                </Link>
              </li>
            ),
          )}
        </ul>
      </nav>
    </article>
  );
}
