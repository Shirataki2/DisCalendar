"use client";

import { CheckIcon, ChevronLeftIcon, CopyIcon } from "lucide-react";
import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import { useEffect, useState } from "react";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
  GUILD_SETTINGS_SECTIONS,
  guildCalendarPath,
  guildSettingsPath,
} from "@/lib/guild-settings";
import { calendarDateParam } from "@/lib/login-redirect";
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
  // カレンダーで表示していた日付 (#440)。不正な値は捨てる
  const date = calendarDateParam(useSearchParams().get("date"));
  const canManage = useCanManageServer(guildId);
  return (
    <main lang={language} className="min-h-0 flex-1 overflow-y-auto p-4 sm:p-6">
      <div className="mx-auto flex max-w-5xl flex-col gap-4">
        <Link
          href={guildCalendarPath(guildId, date)}
          className="inline-flex min-h-11 items-center gap-1 self-start text-sm underline-offset-4 hover:underline"
        >
          <ChevronLeftIcon aria-hidden className="size-4" />
          {t("カレンダーに戻る")}
        </Link>
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
          <div className="min-w-0">
            <h1 className="text-2xl font-bold">{t("サーバー設定")}</h1>
            <p className="break-words text-sm text-muted-foreground">
              {guildName}
            </p>
          </div>
          <CopyGuildIdButton guildId={guildId} />
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
            className="md:sticky md:top-0 md:w-48 md:shrink-0"
          >
            {/* スマホでは折り返して全項目を見せる (横スクロールだと端の項目に気づきにくい) */}
            <ul className="flex flex-wrap gap-1 md:flex-col">
              {GUILD_SETTINGS_SECTIONS.map((section) => {
                const href = guildSettingsPath(guildId, section.slug, date);
                const current =
                  pathname === guildSettingsPath(guildId, section.slug);
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

/**
 * サーバー ID (Discord の Snowflake。文字列のまま扱う) をクリップボードにコピーする。
 * API トークンでの連携や問い合わせで ID を求められたときに、Discord の開発者モードを使わずに済むようにする
 */
function CopyGuildIdButton({ guildId }: { guildId: string }) {
  const { t } = useLanguage();
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");

  // 「コピーしました」は数秒で元の表示に戻す
  useEffect(() => {
    if (state !== "copied") return;
    const timer = setTimeout(() => setState("idle"), 2000);
    return () => clearTimeout(timer);
  }, [state]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(guildId);
      setState("copied");
    } catch {
      // 権限が無い / 非セキュアなコンテキストなど。ID を表示して手動でコピーしてもらう
      setState("failed");
    }
  };

  return (
    <div className="flex flex-col items-start gap-1">
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => void copy()}
        title={t("サーバー ID: {id}", { id: guildId })}
        className="min-h-9"
      >
        {state === "copied" ? (
          <CheckIcon data-icon="inline-start" />
        ) : (
          <CopyIcon data-icon="inline-start" />
        )}
        {state === "copied" ? t("コピーしました") : t("サーバー ID をコピー")}
      </Button>
      {/* 結果を読み上げる (ボタンの文言の切り替えだけでは伝わらない支援技術向け) */}
      <p role="status" className="text-xs text-muted-foreground">
        {state === "copied" ? (
          <span className="sr-only">{t("サーバー ID をコピーしました")}</span>
        ) : state === "failed" ? (
          <>
            {t("コピーできませんでした。ID を選択してコピーしてください:")}{" "}
            <span className="select-all font-mono">{guildId}</span>
          </>
        ) : null}
      </p>
    </div>
  );
}
