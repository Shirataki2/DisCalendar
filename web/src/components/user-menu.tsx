"use client";

import {
  CalendarCogIcon,
  CalendarDaysIcon,
  ChevronDownIcon,
  LayoutGridIcon,
  LogOutIcon,
} from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { CalendarSettingsDialog } from "@/components/calendar-settings-dialog";
import { ThemeToggleContent, useThemeToggle } from "@/components/theme-toggle";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useSignOut } from "@/hooks/use-sign-out";
import { ROUTES } from "@/lib/site";

interface Props {
  name: string;
  image: string | null;
}

/**
 * ヘッダ右端のアカウントメニュー (旧実装の AccountMenu.vue 相当)。
 * アバターを押すとドロップダウンで「サーバー一覧」「すべての予定」「カレンダーの表示設定」「テーマ切替」「ログアウト」を出す
 */
export function UserMenu({ name, image }: Props) {
  const signOut = useSignOut();
  const toggleTheme = useThemeToggle();
  // ダイアログはメニューが閉じても残るよう、メニューの外に置いて開閉だけここで持つ
  const [calendarSettingsOpen, setCalendarSettingsOpen] = useState(false);

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button
              type="button"
              variant="ghost"
              size="lg"
              aria-label="アカウントメニュー"
              className="rounded-full pr-2 pl-1.5"
            />
          }
        >
          {image ? (
            // biome-ignore lint/performance/noImgElement: Discord CDN のアバターは最適化不要
            <img src={image} alt="" className="size-7 rounded-full" />
          ) : (
            <span className="flex size-7 items-center justify-center rounded-full bg-foreground/10 text-xs font-bold">
              {name.slice(0, 1)}
            </span>
          )}
          <span className="hidden max-w-40 truncate text-sm font-medium sm:inline">
            {name}
          </span>
          <ChevronDownIcon
            className="size-4 text-muted-foreground"
            aria-hidden
          />
        </DropdownMenuTrigger>
        {/* 幅は「ライトテーマに切り替え」が 1 行に収まるように取る */}
        <DropdownMenuContent align="end" className="min-w-56">
          {/* ヘッダに名前を出さないスマホ幅ではメニューの先頭に出す */}
          <DropdownMenuGroup className="sm:hidden">
            <DropdownMenuLabel className="truncate">{name}</DropdownMenuLabel>
          </DropdownMenuGroup>
          <DropdownMenuSeparator className="sm:hidden" />
          <DropdownMenuItem render={<Link href={ROUTES.dashboard} />}>
            <LayoutGridIcon aria-hidden />
            サーバー一覧
          </DropdownMenuItem>
          <DropdownMenuItem render={<Link href={ROUTES.dashboardAll} />}>
            <CalendarDaysIcon aria-hidden />
            すべての予定
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => setCalendarSettingsOpen(true)}>
            <CalendarCogIcon aria-hidden />
            カレンダーの表示設定
          </DropdownMenuItem>
          <DropdownMenuItem onClick={toggleTheme}>
            <ThemeToggleContent />
          </DropdownMenuItem>
          <DropdownMenuItem onClick={() => void signOut()}>
            <LogOutIcon aria-hidden />
            ログアウト
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      {/* メニューを閉じてもダイアログが残るよう、メニューの外に置く */}
      <CalendarSettingsDialog
        open={calendarSettingsOpen}
        onOpenChange={setCalendarSettingsOpen}
      />
    </>
  );
}
