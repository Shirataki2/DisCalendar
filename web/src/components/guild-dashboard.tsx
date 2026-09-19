"use client";

import { CheckIcon, ChevronsUpDownIcon, SettingsIcon } from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { EventCalendar } from "@/components/event-calendar";
import { GuildSettingsDialog } from "@/components/guild-settings-dialog";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { Guild } from "@/lib/api/types";
import {
  canEditEvents,
  useGuildConfigQuery,
  useMyPermissionsQuery,
  useRefreshMyPermissions,
} from "@/lib/query/guild";
import { ROUTES } from "@/lib/site";

interface Props {
  guild: Guild;
  guildChoices: GuildChoice[];
  initialDate?: string;
  initialEventId?: number;
}

export interface GuildChoice {
  id: string;
  name: string;
  iconUrl: string | null;
  accessLabel: string | null;
}

/**
 * ギルドのカレンダー画面 (見出し + カレンダー + サーバー設定ダイアログ)。
 * restricted 設定と自分の権限は TanStack Query のキャッシュから読むので、
 * サーバー設定ダイアログで保存するとカレンダーの編集可否がその場で切り替わる
 */
export function GuildDashboard({
  guild,
  guildChoices,
  initialDate,
  initialEventId,
}: Props) {
  const guildId = guild.guild_id;
  const configQuery = useGuildConfigQuery(guildId);
  const permissionsQuery = useMyPermissionsQuery(guildId);
  const refreshPermissions = useRefreshMyPermissions(guildId);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const canEdit = canEditEvents(
    permissionsQuery.isError ? undefined : permissionsQuery.data,
  );
  // 連携の可否は、取得できていないときだけでなく**取得に失敗したとき**も無効側に倒す (#122)。
  // Bot がサーバーから外れているとこのクエリ自体が 403 になり、TanStack Query は
  // 直前に成功した「権限あり」を持ち続けるため、そのままだと操作できるように見えてしまう
  const discordPermissions = permissionsQuery.isError
    ? undefined
    : permissionsQuery.data;

  return (
    <main className="flex min-h-0 flex-1 flex-col gap-2 p-4">
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-x-3 gap-y-1">
        {configQuery.data?.restricted && !canEdit && (
          <span className="rounded-md border border-indigo-400/30 bg-indigo-500/10 px-3 py-2 text-xs text-muted-foreground">
            このサーバーでは管理権限または指定ロールを持つメンバーが予定を編集できます
            <Link
              href={ROUTES.tutorial}
              className="ml-2 inline-block font-medium text-foreground underline underline-offset-4"
            >
              練習用カレンダーで操作を試す
            </Link>
          </span>
        )}
        <div className="flex min-w-0 items-center gap-1">
          <GuildSwitcher currentGuildId={guildId} guilds={guildChoices} />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label="サーバー設定"
            title="サーバー設定"
            onClick={() => setSettingsOpen(true)}
            className="shrink-0"
          >
            <SettingsIcon />
          </Button>
        </div>
      </div>
      <EventCalendar
        guildName={guild.name ?? guildId}
        guildId={guildId}
        canEdit={canEdit}
        initialDate={initialDate}
        initialEventId={initialEventId}
        // 新規作成の事前通知の初期値はサーバー設定 (#181)。RSC で hydrate 済みなので通常は取れている
        defaultNotifications={configQuery.data?.default_notifications}
        // 権限を取得できるまでは無効 (disabled + 案内) 側に倒す
        discordSync={{
          botCreateEvents: discordPermissions?.bot_create_events ?? false,
          canCreateEvents: discordPermissions?.create_events ?? false,
          // Bot の招待し直しやロール付与の直後でも待たずに反映できるようにする (#122)
          onRefresh: () => refreshPermissions.mutateAsync(),
        }}
      />
      <GuildSettingsDialog
        guildId={guildId}
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
    </main>
  );
}

function GuildSwitcher({
  currentGuildId,
  guilds,
}: {
  currentGuildId: string;
  guilds: GuildChoice[];
}) {
  const current = guilds.find((guild) => guild.id === currentGuildId);
  if (!current) return null;

  const content = (
    <>
      <GuildIcon guild={current} className="size-8" />
      <span
        className="min-w-0 truncate text-lg font-semibold"
        title={current.name}
      >
        {current.name}
      </span>
    </>
  );

  if (guilds.length === 1) {
    return (
      <div className="flex min-w-0 items-center gap-3 px-2">{content}</div>
    );
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        aria-label={`サーバーを切り替え: ${current.name}`}
        className="flex h-11 min-w-0 max-w-full items-center gap-3 rounded-md px-2 outline-none hover:bg-foreground/10 focus-visible:ring-2 focus-visible:ring-ring"
      >
        {content}
        <ChevronsUpDownIcon
          className="size-4 shrink-0 text-muted-foreground"
          aria-hidden
        />
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="max-h-[min(24rem,var(--available-height))] w-80 max-w-[calc(100vw-2rem)]"
      >
        <DropdownMenuGroup>
          <DropdownMenuLabel>サーバーを切り替え</DropdownMenuLabel>
          {guilds.map((choice) => {
            const selected = choice.id === currentGuildId;
            const itemContent = (
              <>
                <GuildIcon guild={choice} className="size-8" />
                <span className="min-w-0 flex-1">
                  <span
                    className="block truncate font-medium"
                    title={choice.name}
                  >
                    {choice.name}
                  </span>
                  {choice.accessLabel && (
                    <span className="block text-xs text-muted-foreground">
                      {choice.accessLabel}
                    </span>
                  )}
                </span>
                {selected && <CheckIcon className="size-4" aria-hidden />}
              </>
            );
            return selected ? (
              <DropdownMenuItem
                key={choice.id}
                disabled
                aria-current="page"
                className="min-h-11 px-2 py-1.5 opacity-100"
              >
                {itemContent}
              </DropdownMenuItem>
            ) : (
              <DropdownMenuItem
                key={choice.id}
                render={
                  <Link href={`/dashboard/${choice.id}`} prefetch={false} />
                }
                className="min-h-11 px-2 py-1.5"
              >
                {itemContent}
              </DropdownMenuItem>
            );
          })}
        </DropdownMenuGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function GuildIcon({
  guild,
  className,
}: {
  guild: GuildChoice;
  className: string;
}) {
  return guild.iconUrl ? (
    // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
    <img
      src={guild.iconUrl}
      alt=""
      className={`${className} shrink-0 rounded-full`}
    />
  ) : (
    <span
      className={`${className} flex shrink-0 items-center justify-center rounded-full bg-foreground/10 font-bold`}
      aria-hidden
    >
      {guild.name.slice(0, 1)}
    </span>
  );
}
