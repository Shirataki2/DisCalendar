import Link from "next/link";
import { Logo } from "@/components/logo";
import { GITHUB_URL, ROUTES, SUPPORT_SERVER_URL } from "@/lib/site";

const INTERNAL_LINKS = [
  { href: ROUTES.docs, label: "使い方" },
  { href: ROUTES.tos, label: "利用規約" },
  { href: ROUTES.privacy, label: "プライバシーポリシー" },
] as const;

const EXTERNAL_LINKS = [
  { href: SUPPORT_SERVER_URL, label: "サポートサーバー" },
  { href: GITHUB_URL, label: "GitHub" },
] as const;

/** 公開ページ (LP / docs / 規約) 共通のフッタ */
export function SiteFooter() {
  return (
    <footer className="border-t border-white/10">
      <div className="mx-auto flex max-w-6xl flex-col gap-6 px-4 py-8 text-sm text-neutral-400 sm:flex-row sm:items-center sm:justify-between sm:px-6">
        <div className="flex flex-col gap-2">
          <Logo className="text-lg text-neutral-200" />
          <p className="text-xs">&copy; 2021 DisCalendar</p>
        </div>
        <nav aria-label="フッタのリンク">
          <ul className="flex flex-wrap gap-x-5 gap-y-2">
            {INTERNAL_LINKS.map((link) => (
              <li key={link.href}>
                <Link
                  href={link.href}
                  className="transition-colors hover:text-white"
                >
                  {link.label}
                </Link>
              </li>
            ))}
            {EXTERNAL_LINKS.map((link) => (
              <li key={link.href}>
                <a
                  href={link.href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="transition-colors hover:text-white"
                >
                  {link.label}
                </a>
              </li>
            ))}
          </ul>
        </nav>
      </div>
    </footer>
  );
}
