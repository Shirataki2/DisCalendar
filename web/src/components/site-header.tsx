import Link from "next/link";
import { Logo } from "@/components/logo";
import { SessionLink } from "@/components/session-link";
import { ROUTES } from "@/lib/site";

const NAV_LINKS = [
  { href: ROUTES.docs, label: "使い方" },
  { href: ROUTES.changelog, label: "更新履歴" },
  { href: ROUTES.tos, label: "利用規約" },
  { href: ROUTES.privacy, label: "プライバシーポリシー" },
] as const;

/** 公開ページ (LP / docs / 規約) 共通のヘッダ */
export function SiteHeader() {
  return (
    <header className="sticky top-0 z-20 border-b border-white/10 bg-background/80 backdrop-blur">
      <div className="mx-auto flex h-14 max-w-6xl items-center gap-6 px-4 sm:px-6">
        <Link href="/" className="shrink-0" aria-label="DisCalendar ホーム">
          <Logo className="text-xl" />
        </Link>
        {/* 640px 前後だとロゴ + 4 項目 + 右のボタンが h-14 に収まらず折り返すので、md から出す (それ未満はフッタに同じリンクがある) */}
        <nav aria-label="サイト内リンク" className="hidden md:block">
          <ul className="flex items-center gap-5 text-sm text-neutral-300">
            {NAV_LINKS.map((link) => (
              <li key={link.href}>
                <Link
                  href={link.href}
                  className="transition-colors hover:text-white"
                >
                  {link.label}
                </Link>
              </li>
            ))}
          </ul>
        </nav>
        <div className="ml-auto">
          <SessionLink className="rounded-full border border-white/20 px-4 py-1.5 text-sm transition-colors hover:bg-white/10" />
        </div>
      </div>
    </header>
  );
}
