"use client";

import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react";
import Link from "next/link";
import { useLanguage } from "@/components/language-provider";
import { adjacentDocPages, type DocPage, docPath } from "@/lib/docs";

/** 本文の末尾に出す前後ページのカード (旧 _slug.vue の「前の記事」「次の記事」) */
export function DocsPager({ slug }: { slug: string }) {
  const { t } = useLanguage();
  const { prev, next } = adjacentDocPages(slug);
  return (
    <nav
      aria-label={t("前後のページ")}
      className="mt-12 grid gap-4 border-t border-white/10 pt-8 sm:grid-cols-2"
    >
      {prev ? <PagerCard page={prev} direction="prev" /> : <span />}
      {next && <PagerCard page={next} direction="next" />}
    </nav>
  );
}

function PagerCard({
  page,
  direction,
}: {
  page: DocPage;
  direction: "prev" | "next";
}) {
  const { t } = useLanguage();
  const next = direction === "next";
  return (
    <Link
      href={docPath(page.slug)}
      className={`flex flex-col gap-1 rounded-xl border border-white/10 bg-surface px-5 py-4 transition-colors hover:border-indigo-400/40 hover:bg-white/5 ${
        next ? "items-end text-right sm:col-start-2" : "items-start"
      }`}
    >
      <span className="text-xs text-neutral-400">
        {next ? t("次のページ") : t("前のページ")}
      </span>
      <span className="flex items-center gap-1 font-semibold">
        {!next && <ChevronLeftIcon className="size-4" aria-hidden />}
        {t(page.title)}
        {next && <ChevronRightIcon className="size-4" aria-hidden />}
      </span>
    </Link>
  );
}
