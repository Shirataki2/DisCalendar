"use client";

import {
  BookOpenIcon,
  CalendarCogIcon,
  CircleHelpIcon,
  CodeIcon,
  ExternalLinkIcon,
  HeartIcon,
  HistoryIcon,
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
import { ThemeToggleContent, useThemeToggle } from "@/components/theme-toggle";
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

// 旧実装 (components/header/NavDrawer.vue) と同じ並び (「更新履歴」「支援」は v3 で追加)。
// 旧版と同じくテーマ切替は一覧の最後 (旧版の bottomItems 相当) に置く。
// 「今日へ移動」は FullCalendar のツールバーに「今日」があるため入れていない。
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
  { label: "更新履歴", icon: HistoryIcon, href: ROUTES.changelog },
  { label: "支援", icon: HeartIcon, href: ROUTES.donation },
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

// 現在地の indigo は、ライトでは薄すぎて読めないので濃い側に振る
const itemClass =
  "flex w-full items-center gap-3 rounded-md px-3 py-2 text-sm text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground aria-[current=page]:bg-indigo-500/15 aria-[current=page]:text-indigo-700 dark:aria-[current=page]:text-indigo-300";

interface Props {
  /** 管理コンソールへのリンクを出すか */
  admin: boolean;
  /** リンクを押したとき (スマホのドロワーを閉じるのに使う) */
  onNavigate?: () => void;
  /**
   * 「カレンダーの表示設定」を押したとき。ダイアログ本体は DashboardShell が持つ
   * (ここに置くと、スマホのドロワー (Sheet) が閉じたときに一緒にアンマウントされて開けない)
   */
  onOpenCalendarSettings: () => void;
  className?: string;
}

/**
 * ダッシュボードのナビゲーションドロワーの中身 (旧実装の NavDrawer.vue 相当)。
 * PC の常設サイドバーとスマホのオーバーレイ (Sheet) の両方で使う
 */
export function DashboardNav({
  admin,
  onNavigate,
  onOpenCalendarSettings,
  className,
}: Props) {
  const pathname = usePathname();
  const signOut = useSignOut();
  const toggleTheme = useThemeToggle();

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
                  className="size-3.5 text-muted-foreground"
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
          {/* ダイアログがドロワー (Sheet) と重ならないよう、開くときにドロワーは閉じる */}
          <button
            type="button"
            className={itemClass}
            onClick={() => {
              onNavigate?.();
              onOpenCalendarSettings();
            }}
          >
            <CalendarCogIcon className="size-5 shrink-0" aria-hidden />
            <span className="flex-1 text-left">カレンダーの表示設定</span>
          </button>
        </li>
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
        <li>
          {/* テーマの切替はドロワーを開いたまま結果を確かめられるよう onNavigate を呼ばない */}
          <button type="button" className={itemClass} onClick={toggleTheme}>
            <ThemeToggleContent
              iconClassName="size-5 shrink-0"
              labelClassName="flex-1 text-left"
            />
          </button>
        </li>
      </ul>
    </nav>
  );
}
