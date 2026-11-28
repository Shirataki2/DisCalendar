"use client";

import { ChevronLeftIcon } from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useLanguage } from "@/components/language-provider";
import {
  GUILD_SETTINGS_SECTIONS,
  guildSettingsPath,
} from "@/lib/guild-settings";
import { useCanManageServer } from "@/lib/query/guild";
import { cn } from "@/lib/utils";

/**
 * サーバー設定ページ (`/dashboard/[id]/settings/*`) の共通の枠。項目が増えてダイアログに収まらなくなったので、
 * 節ごとにページを分け、左 (スマホでは上) の一覧から切り替える
 */
export function GuildSettingsShell({
  guildId,
  guildName,
  children,
}: {
  guildId: string;
  guildName: string;
  children: React.ReactNode;
}) {
  const { t, language } = useLanguage();
  const pathname = usePathname();
  const canManage = useCanManageServer(guildId);
  return (
    <main lang={language} className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
      <div className="mx-auto flex max-w-5xl flex-col gap-4">
        <Link
          href={`/dashboard/${guildId}`}
          className="inline-flex min-h-11 items-center gap-1 self-start text-sm underline-offset-4 hover:underline"
        >
          <ChevronLeftIcon aria-hidden className="size-4" />
          {t("カレンダーに戻る")}
        </Link>
        <div>
          <h1 className="text-2xl font-bold">{t("サーバー設定")}</h1>
          <p className="break-words text-sm text-muted-foreground">
            {guildName}
          </p>
        </div>

        {/* 管理権限が無ければ閲覧だけであることを先に伝える (変更は API 側でも 403 になる) */}
        {canManage === false && (
          <p className="rounded-md bg-muted px-3 py-2 text-sm text-muted-foreground">
            {t("サーバーの設定の変更には管理権限が必要です")}
          </p>
        )}

        <div className="flex flex-col gap-6 md:flex-row md:items-start">
          <nav
            aria-label={t("サーバー設定の項目")}
            className="-mx-4 overflow-x-auto px-4 md:sticky md:top-0 md:mx-0 md:w-48 md:shrink-0 md:px-0"
          >
            <ul className="flex gap-1 md:flex-col">
              {GUILD_SETTINGS_SECTIONS.map((section) => {
                const href = guildSettingsPath(guildId, section.slug);
                const current = pathname === href;
                return (
                  <li key={section.slug} className="shrink-0">
                    <Link
                      href={href}
                      aria-current={current ? "page" : undefined}
                      className={cn(
                        "flex min-h-11 items-center rounded-md px-3 text-sm whitespace-nowrap hover:bg-foreground/10",
                        current && "bg-foreground/10 font-medium",
                      )}
                    >
                      {t(section.label)}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </nav>
          <div className="min-w-0 max-w-3xl flex-1">{children}</div>
        </div>
      </div>
    </main>
  );
}
