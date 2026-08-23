"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ROUTES } from "@/lib/site";

// 管理コンソールのメニュー。href が無い項目はリンクにせず「準備中」と出す
// (存在しないパスへ飛ばして 404 にしない)
const ITEMS: { label: string; href?: string }[] = [
  { label: "概要", href: ROUTES.admin },
  { label: "ギルド・予定", href: ROUTES.adminGuilds },
  { label: "SQL・定型操作", href: ROUTES.adminSql },
  { label: "ユーザー", href: ROUTES.adminUsers },
  { label: "監査ログ", href: ROUTES.adminAuditLogs },
];

/** 概要 (/admin) は完全一致、それ以外は配下のページ (/admin/guilds/123 など) も現在地扱い */
function isCurrent(pathname: string, href: string): boolean {
  if (href === ROUTES.admin) return pathname === href;
  return pathname === href || pathname.startsWith(`${href}/`);
}

export function AdminNav() {
  const pathname = usePathname();
  return (
    <nav aria-label="管理コンソール" className="flex flex-wrap gap-1 text-sm">
      {ITEMS.map((item) =>
        item.href ? (
          <Link
            key={item.label}
            href={item.href}
            aria-current={isCurrent(pathname, item.href) ? "page" : undefined}
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
