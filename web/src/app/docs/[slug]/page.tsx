import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { DocsPager } from "@/components/docs/docs-pager";
import { DOC_PAGES, docPath, findDocPage } from "@/lib/docs";

// 使い方ページ。本文は src/content/docs/<slug>.mdx にあり、DOC_PAGES の分だけビルド時に静的生成する
// (それ以外の slug は dynamicParams = false で 404)。URL は旧実装 (@nuxt/content) の /docs/<slug> のまま

export const dynamicParams = false;

export function generateStaticParams() {
  return DOC_PAGES.map((page) => ({ slug: page.slug }));
}

export async function generateMetadata({
  params,
}: PageProps<"/docs/[slug]">): Promise<Metadata> {
  const { slug } = await params;
  const page = findDocPage(slug);
  if (!page) return {};
  return {
    title: page.title,
    description: page.description,
    openGraph: {
      title: page.title,
      description: page.description,
      url: docPath(page.slug),
    },
  };
}

export default async function DocPage({ params }: PageProps<"/docs/[slug]">) {
  const { slug } = await params;
  const page = findDocPage(slug);
  if (!page) notFound();

  // slug は DOC_PAGES の値に限られる (dynamicParams = false) ので、動的 import の対象も content/docs 配下に閉じる
  const { default: Content } = await import(`@/content/docs/${slug}.mdx`);

  return (
    <article className="max-w-3xl">
      <header className="mb-8 border-b border-white/10 pb-6">
        <p className="mb-2 text-sm text-neutral-400">使い方</p>
        <h1 className="text-3xl font-bold sm:text-4xl">{page.title}</h1>
        <p className="mt-3 text-sm leading-7 text-neutral-400">
          {page.description}
        </p>
      </header>
      <Content />
      <DocsPager slug={slug} />
    </article>
  );
}
