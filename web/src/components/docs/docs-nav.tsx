"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useRef } from "react";
import { DOC_PAGES, docPath } from "@/lib/docs";
import { cn } from "@/lib/utils";

interface Props {
  /** aside (PC のサイドバー) か、本文上部の横並び (スマホ) か */
  orientation: "vertical" | "horizontal";
}

/** docs のページ一覧ナビ。現在のページは usePathname で判定して強調する */
export function DocsNav({ orientation }: Props) {
  const pathname = usePathname();
  const vertical = orientation === "vertical";
  const activeRef = useRef<HTMLAnchorElement>(null);

  // 横並び (スマホ) では現在のページが画面外に隠れがちなので、表示時にスクロールして見せる
  useEffect(() => {
    if (!vertical) {
      activeRef.current?.scrollIntoView({ inline: "center", block: "nearest" });
    }
  }, [vertical]);
  return (
    <nav aria-label="使い方の目次">
      <ol
        className={cn(
          "flex gap-1 text-sm",
          vertical
            ? "flex-col"
            : "snap-x flex-row overflow-x-auto pb-2 [scrollbar-width:thin]",
        )}
      >
        {DOC_PAGES.map((page, index) => {
          const href = docPath(page.slug);
          const active = pathname === href;
          return (
            <li key={page.slug} className={vertical ? "" : "snap-start"}>
              <Link
                ref={active ? activeRef : undefined}
                href={href}
                aria-current={active ? "page" : undefined}
                className={cn(
                  "flex items-center gap-2.5 rounded-md px-3 py-1.5 whitespace-nowrap transition-colors",
                  active
                    ? "bg-indigo-500/20 font-semibold text-white"
                    : "text-neutral-300 hover:bg-white/5 hover:text-white",
                )}
              >
                <span
                  aria-hidden
                  className={cn(
                    "w-5 shrink-0 text-right font-mono text-xs tabular-nums",
                    active ? "text-indigo-300" : "text-neutral-500",
                  )}
                >
                  {index + 1}
                </span>
                {page.title}
              </Link>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
