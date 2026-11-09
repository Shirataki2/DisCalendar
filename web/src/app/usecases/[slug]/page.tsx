import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { UsecaseContent } from "@/components/usecases/usecase-content";
import { pageOpenGraph } from "@/lib/site";
import { findUsecasePage, USECASE_PAGES, usecasePath } from "@/lib/usecases";

// 用途別の紹介ページ (#381)。中身は lib/usecases.ts の一覧にあり、その分だけビルド時に静的生成する
// (それ以外の slug は dynamicParams = false で 404)

export const dynamicParams = false;

export function generateStaticParams() {
  return USECASE_PAGES.map((page) => ({ slug: page.slug }));
}

export async function generateMetadata({
  params,
}: PageProps<"/usecases/[slug]">): Promise<Metadata> {
  const { slug } = await params;
  const page = findUsecasePage(slug);
  if (!page) return {};
  return {
    title: page.metaTitle,
    description: page.description,
    openGraph: pageOpenGraph({
      title: page.metaTitle,
      description: page.description,
      url: usecasePath(page.slug),
    }),
  };
}

export default async function UsecasePage({
  params,
}: PageProps<"/usecases/[slug]">) {
  const { slug } = await params;
  if (!findUsecasePage(slug)) notFound();
  return (
    <>
      <SiteHeader />
      <UsecaseContent slug={slug} />
      <SiteFooter />
    </>
  );
}
