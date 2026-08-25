"use client";

import { MenuIcon } from "lucide-react";
import Link from "next/link";
import { type ReactNode, useEffect, useState } from "react";
import { DashboardNav } from "@/components/dashboard-nav";
import { Logo } from "@/components/logo";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { SIDEBAR_COOKIE } from "@/lib/dashboard-sidebar";
import { ROUTES } from "@/lib/site";

/** Tailwind の lg ブレークポイント。これ以上なら常設サイドバー、未満ならオーバーレイのドロワー */
const DESKTOP_QUERY = "(min-width: 64rem)";

interface Props {
  /** 管理コンソールへのリンクを出すか */
  admin: boolean;
  /** PC のサイドバーの初期状態 (cookie から。サーバー側で決めてちらつきを防ぐ) */
  defaultSidebarOpen: boolean;
  /** ヘッダ右端のアカウントメニュー (サーバー側で session を読んで作る) */
  user: ReactNode;
  children: ReactNode;
}

/**
 * ダッシュボードのアプリバーとナビゲーションドロワー (旧実装の AppHeader.vue + NavDrawer.vue 相当)。
 * - PC (lg 以上): ハンバーガーで開閉する常設サイドバー。既定は旧版と同じく開いた状態で、開閉は cookie に覚える
 * - それ未満: ハンバーガーで左から出るオーバーレイ (Sheet)。リンクを押すと閉じる
 * フッタは layout 側でこの下に置く (サイドバーより下まで全幅で出すため)
 */
export function DashboardShell({
  admin,
  defaultSidebarOpen,
  user,
  children,
}: Props) {
  const [sidebarOpen, setSidebarOpen] = useState(defaultSidebarOpen);
  const [drawerOpen, setDrawerOpen] = useState(false);

  const toggleSidebar = () => {
    const next = !sidebarOpen;
    setSidebarOpen(next);
    // biome-ignore lint/suspicious/noDocumentCookie: Cookie Store API は Safari / Firefox の少し前の版に無いので document.cookie を使う
    document.cookie = `${SIDEBAR_COOKIE}=${next ? "open" : "closed"}; path=/; max-age=31536000; samesite=lax`;
  };

  // ドロワーを開いたまま PC 幅に広げたときは閉じる (サイドバーと二重に出さない)
  useEffect(() => {
    const media = window.matchMedia(DESKTOP_QUERY);
    const onChange = (event: MediaQueryListEvent) => {
      if (event.matches) setDrawerOpen(false);
    };
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  return (
    <>
      <header className="flex h-14 shrink-0 items-center gap-1 border-b border-border px-2 sm:px-3">
        {/* 2 つのボタンは CSS で出し分ける (PC はサイドバーの開閉、スマホはドロワーを開く) */}
        <Button
          type="button"
          variant="ghost"
          size="icon-lg"
          aria-label="メニュー"
          aria-expanded={sidebarOpen}
          aria-controls="dashboard-sidebar"
          onClick={toggleSidebar}
          className="hidden lg:inline-flex"
        >
          <MenuIcon className="size-5" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-lg"
          aria-label="メニュー"
          onClick={() => setDrawerOpen(true)}
          className="lg:hidden"
        >
          <MenuIcon className="size-5" />
        </Button>
        <Link
          href={ROUTES.dashboard}
          aria-label="サーバー一覧へ"
          className="ml-1 sm:ml-2"
        >
          <Logo className="text-xl" />
        </Link>
        <div className="ml-auto">{user}</div>
      </header>
      <div className="flex min-h-0 flex-1">
        <aside
          id="dashboard-sidebar"
          className={sidebarOpen ? "hidden lg:block" : "hidden"}
        >
          <div className="h-full w-64 overflow-y-auto border-r border-border bg-surface">
            <DashboardNav admin={admin} />
          </div>
        </aside>
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">{children}</div>
      </div>
      <Sheet open={drawerOpen} onOpenChange={setDrawerOpen}>
        <SheetContent side="left" className="gap-0 data-[side=left]:w-64">
          <SheetHeader className="h-14 flex-row items-center border-b border-border px-4 py-0">
            <SheetTitle>
              <Logo className="text-xl" />
            </SheetTitle>
            <SheetDescription className="sr-only">
              サイト内メニュー
            </SheetDescription>
          </SheetHeader>
          <DashboardNav
            admin={admin}
            onNavigate={() => setDrawerOpen(false)}
            className="overflow-y-auto"
          />
        </SheetContent>
      </Sheet>
    </>
  );
}
