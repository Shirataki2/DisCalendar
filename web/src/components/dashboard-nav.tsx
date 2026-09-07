"use client";

import {
  BellIcon,
  BookOpenIcon,
  CalendarCogIcon,
  CalendarDaysIcon,
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
import { useId } from "react";
import { ThemeToggleContent, useThemeToggle } from "@/components/theme-toggle";
import { useSignOut } from "@/hooks/use-sign-out";
import { GITHUB_URL, ROUTES, SUPPORT_SERVER_URL } from "@/lib/site";
import { cn } from "@/lib/utils";

const SECTIONS = [
  "メイン",
  "設定",
  "サポート",
  "サービス情報",
  "管理",
] as const;

interface NavItem {
  section: (typeof SECTIONS)[number];
  label: string;
  icon: LucideIcon;
  href: string;
  /** 別タブで開く外部リンク */
  external?: boolean;
  /** 管理者 (api の ADMIN_DISCORD_USER_IDS) にだけ出す */
  adminOnly?: boolean;
}

// PC とスマホで共通の分類・表示順。現在地の判定にも同じリンク一覧を使う。
const ITEMS: NavItem[] = [
  { label: "ホーム", section: "メイン", icon: HouseIcon, href: ROUTES.home },
  {
    label: "サーバー一覧",
    section: "メイン",
    icon: LayoutGridIcon,
    href: ROUTES.dashboard,
  },
  {
    label: "すべての予定",
    section: "メイン",
    icon: CalendarDaysIcon,
    href: ROUTES.dashboardAll,
  },
  {
    label: "使い方",
    section: "サポート",
    icon: CircleHelpIcon,
    href: ROUTES.docs,
  },
  {
    label: "サポートサーバー",
    section: "サポート",
    icon: LifeBuoyIcon,
    href: SUPPORT_SERVER_URL,
    external: true,
  },
  {
    label: "更新履歴",
    section: "サービス情報",
    icon: HistoryIcon,
    href: ROUTES.changelog,
  },
  {
    label: "支援",
    section: "サービス情報",
    icon: HeartIcon,
    href: ROUTES.donation,
  },
  {
    label: "GitHub",
    section: "サービス情報",
    icon: CodeIcon,
    href: GITHUB_URL,
    external: true,
  },
  {
    label: "利用規約",
    section: "サービス情報",
    icon: BookOpenIcon,
    href: ROUTES.tos,
  },
  {
    label: "プライバシーポリシー",
    section: "サービス情報",
    icon: ShieldCheckIcon,
    href: ROUTES.privacy,
  },
  {
    label: "管理コンソール",
    section: "管理",
    icon: WrenchIcon,
    href: ROUTES.admin,
    adminOnly: true,
  },
];

/** リンク先の「配下」の基準。docs は /docs/gettingstarted へのリンクだが、他の docs ページでも現在地にする */
function baseOf(href: string): string {
  return href.startsWith("/docs/") ? "/docs" : href;
}

function isUnder(pathname: string, base: string): boolean {
  return pathname === base || pathname.startsWith(`${base}/`);
}

/**
 * ホーム (/) は完全一致、それ以外は配下のページ (/dashboard/123、/docs/xxx など) も現在地扱い。
 * ただし配下に別の項目がある場合 (/dashboard/all は「サーバー一覧」の配下でもある) は、
 * 一番長く一致する項目だけを現在地にする
 */
function isCurrent(pathname: string, href: string): boolean {
  if (href === ROUTES.home) return pathname === href;
  const base = baseOf(href);
  if (!isUnder(pathname, base)) return false;
  return !ITEMS.some((item) => {
    const other = baseOf(item.href);
    return (
      other !== base && other.length > base.length && isUnder(pathname, other)
    );
  });
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
  onOpenPushSettings: () => void;
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
  onOpenPushSettings,
  className,
}: Props) {
  const pathname = usePathname();
  const headingId = useId();
  const signOut = useSignOut();
  const toggleTheme = useThemeToggle();

  return (
    <nav
      aria-label="サイト内メニュー"
      className={cn("flex flex-col gap-4 p-2", className)}
    >
      {SECTIONS.filter((section) => admin || section !== "管理").map(
        (section) => (
          <section key={section} aria-labelledby={`${headingId}-${section}`}>
            <h2
              id={`${headingId}-${section}`}
              className="px-3 pb-1 pt-2 text-xs font-medium text-muted-foreground"
            >
              {section}
            </h2>
            <ul className="flex flex-col gap-0.5">
              {ITEMS.filter(
                (item) =>
                  item.section === section && (admin || !item.adminOnly),
              ).map((item) => (
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
              {section === "設定" && (
                <>
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
                      <CalendarCogIcon
                        className="size-5 shrink-0"
                        aria-hidden
                      />
                      <span className="flex-1 text-left">
                        カレンダーの表示設定
                      </span>
                    </button>
                  </li>
                  <li>
                    <button
                      type="button"
                      className={itemClass}
                      onClick={() => {
                        onOpenPushSettings();
                        onNavigate?.();
                      }}
                    >
                      <BellIcon className="size-5 shrink-0" aria-hidden />
                      <span>プッシュ通知</span>
                    </button>
                  </li>
                  <li>
                    {/* テーマの切替はドロワーを開いたまま結果を確かめられるよう onNavigate を呼ばない */}
                    <button
                      type="button"
                      className={itemClass}
                      onClick={toggleTheme}
                    >
                      <ThemeToggleContent
                        iconClassName="size-5 shrink-0"
                        labelClassName="flex-1 text-left"
                      />
                    </button>
                  </li>
                </>
              )}
            </ul>
          </section>
        ),
      )}
      <ul className="border-t border-border pt-2">
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
