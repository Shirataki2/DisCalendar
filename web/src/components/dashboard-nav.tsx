"use client";

import {
  BookOpenIcon,
  CircleHelpIcon,
  CodeIcon,
  ExternalLinkIcon,
  HouseIcon,
  LayoutGridIcon,
  LifeBuoyIcon,
  LogOutIcon,
  type LucideIcon,
  ShieldCheckIcon,
  WrenchIcon,
} from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useSignOut } from "@/hooks/use-sign-out";
import { GITHUB_URL, ROUTES, SUPPORT_SERVER_URL } from "@/lib/site";
import { cn } from "@/lib/utils";

interface NavItem {
  label: string;
  icon: LucideIcon;
  href: string;
  /** 別タブで開く外部リンク */
  external?: boolean;
  /** 管理者 (api の ADMIN_DISCORD_USER_IDS) にだけ出す */
  adminOnly?: boolean;
}

// 旧実装 (components/header/NavDrawer.vue) と同じ並び。
// 「テーマ変更」はダーク固定のため、「今日へ移動」は FullCalendar のツールバーに「今日」があるため入れていない。
// 「ダッシュボード」は新 web の他の導線 (SessionLink / 管理コンソール) に合わせて「サーバー一覧」と呼ぶ
const ITEMS: NavItem[] = [
  { label: "ホーム", icon: HouseIcon, href: ROUTES.home },
  { label: "サーバー一覧", icon: LayoutGridIcon, href: ROUTES.dashboard },
  {
    label: "サポートサーバー",
    icon: LifeBuoyIcon,
    href: SUPPORT_SERVER_URL,
    external: true,
  },
  { label: "使い方", icon: CircleHelpIcon, href: ROUTES.docs },
  { label: "利用規約", icon: BookOpenIcon, href: ROUTES.tos },
  {
    label: "プライバシーポリシー",
    icon: ShieldCheckIcon,
    href: ROUTES.privacy,
  },
  { label: "GitHub", icon: CodeIcon, href: GITHUB_URL, external: true },
  {
    label: "管理コンソール",
    icon: WrenchIcon,
    href: ROUTES.admin,
    adminOnly: true,
  },
];

/** ホーム (/) は完全一致、それ以外は配下のページ (/dashboard/123、/docs/xxx など) も現在地扱い */
function isCurrent(pathname: string, href: string): boolean {
  if (href === ROUTES.home) return pathname === href;
  // docs は /docs/gettingstarted へのリンクだが、他の docs ページでも現在地にする
  const base = href.startsWith("/docs/") ? "/docs" : href;
  return pathname === base || pathname.startsWith(`${base}/`);
}

const itemClass =
  "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm text-neutral-300 transition-colors hover:bg-white/10 hover:text-white aria-[current=page]:bg-indigo-500/15 aria-[current=page]:text-indigo-300";

interface Props {
  /** 管理コンソールへのリンクを出すか */
  admin: boolean;
  /** リンクを押したとき (スマホのドロワーを閉じるのに使う) */
  onNavigate?: () => void;
  className?: string;
}

/**
 * ダッシュボードのナビゲーションドロワーの中身 (旧実装の NavDrawer.vue 相当)。
 * PC の常設サイドバーとスマホのオーバーレイ (Sheet) の両方で使う
 */
export function DashboardNav({ admin, onNavigate, className }: Props) {
  const pathname = usePathname();
  const signOut = useSignOut();

  return (
    <nav
      aria-label="サイト内メニュー"
      className={cn("flex flex-col gap-0.5 p-2", className)}
    >
      <ul className="flex flex-col gap-0.5">
        {ITEMS.filter((item) => admin || !item.adminOnly).map((item) => (
          <li key={item.label}>
            {item.external ? (
              <a
                href={item.href}
                target="_blank"
                rel="noopener noreferrer"
                className={itemClass}
                onClick={onNavigate}
              >
                <item.icon className="size-5 shrink-0" aria-hidden />
                <span className="flex-1">{item.label}</span>
                <ExternalLinkIcon
                  className="size-3.5 text-neutral-500"
                  aria-hidden
                />
              </a>
            ) : (
              <Link
                href={item.href}
                aria-current={
                  isCurrent(pathname, item.href) ? "page" : undefined
                }
                className={itemClass}
                onClick={onNavigate}
              >
                <item.icon className="size-5 shrink-0" aria-hidden />
                <span className="flex-1">{item.label}</span>
              </Link>
            )}
          </li>
        ))}
        <li>
          <button
            type="button"
            className={itemClass}
            onClick={() => {
              onNavigate?.();
              void signOut();
            }}
          >
            <LogOutIcon className="size-5 shrink-0" aria-hidden />
            <span className="flex-1 text-left">ログアウト</span>
          </button>
        </li>
      </ul>
    </nav>
  );
}
