"use client";

import { ChevronDownIcon, LayoutGridIcon, LogOutIcon } from "lucide-react";
import Link from "next/link";
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
 * アバターを押すとドロップダウンで「サーバー一覧」「ログアウト」を出す。
 * 旧版にあった「テーマ変更」はダーク固定のため無い
 */
export function UserMenu({ name, image }: Props) {
  const signOut = useSignOut();

  return (
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
          <span className="flex size-7 items-center justify-center rounded-full bg-white/10 text-xs font-bold">
            {name.slice(0, 1)}
          </span>
        )}
        <span className="hidden max-w-40 truncate text-sm font-medium sm:inline">
          {name}
        </span>
        <ChevronDownIcon className="size-4 text-neutral-400" aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="min-w-48">
        {/* ヘッダに名前を出さないスマホ幅ではメニューの先頭に出す */}
        <DropdownMenuGroup className="sm:hidden">
          <DropdownMenuLabel className="truncate">{name}</DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator className="sm:hidden" />
        <DropdownMenuItem render={<Link href={ROUTES.dashboard} />}>
          <LayoutGridIcon aria-hidden />
          サーバー一覧
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => void signOut()}>
          <LogOutIcon aria-hidden />
          ログアウト
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
