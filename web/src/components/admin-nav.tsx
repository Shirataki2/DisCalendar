"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ROUTES } from "@/lib/site";

// 管理コンソールのメニュー。画面は子 Issue (#35〜#37) で順に足すので、未実装のものは
// リンクにせず「準備中」と出す (存在しないパスへ飛ばして 404 にしない)
const ITEMS: { label: string; href?: string }[] = [
  { label: "概要", href: ROUTES.admin },
  { label: "ギルド・予定" },
  { label: "SQL" },
  { label: "ユーザー" },
  { label: "監査ログ" },
];

export function AdminNav() {
  const pathname = usePathname();
  return (
    <nav aria-label="管理コンソール" className="flex flex-wrap gap-1 text-sm">
      {ITEMS.map((item) =>
        item.href ? (
          <Link
            key={item.label}
            href={item.href}
            aria-current={pathname === item.href ? "page" : undefined}
            className="rounded-full px-3 py-1 text-neutral-300 transition-colors hover:bg-white/10 aria-[current=page]:bg-white/15 aria-[current=page]:text-white"
          >
            {item.label}
          </Link>
        ) : (
          <span
            key={item.label}
            aria-disabled="true"
            title="準備中"
            className="rounded-full px-3 py-1 text-neutral-500"
          >
            {item.label}
            <span className="ml-1 text-[10px] uppercase tracking-wide">
              準備中
            </span>
          </span>
        ),
      )}
    </nav>
  );
}
