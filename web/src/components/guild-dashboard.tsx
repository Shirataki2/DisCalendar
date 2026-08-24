"use client";

import { SettingsIcon } from "lucide-react";
import { useState } from "react";
import { EventCalendar } from "@/components/event-calendar";
import { GuildSettingsDialog } from "@/components/guild-settings-dialog";
import { Button } from "@/components/ui/button";
import type { Guild } from "@/lib/api/types";
import {
  canEditEvents,
  useGuildConfigQuery,
  useMyPermissionsQuery,
} from "@/lib/query/guild";

interface Props {
  guild: Guild;
}

/**
 * ギルドのカレンダー画面 (見出し + カレンダー + サーバー設定ダイアログ)。
 * restricted 設定と自分の権限は TanStack Query のキャッシュから読むので、
 * サーバー設定ダイアログで保存するとカレンダーの編集可否がその場で切り替わる
 */
export function GuildDashboard({ guild }: Props) {
  const guildId = guild.guild_id;
  const configQuery = useGuildConfigQuery(guildId);
  const permissionsQuery = useMyPermissionsQuery(guildId);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const canEdit = canEditEvents(configQuery.data, permissionsQuery.data);

  return (
    <main className="flex min-h-0 flex-1 flex-col gap-2 p-4">
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-x-3 gap-y-1">
        {configQuery.data?.restricted && !canEdit && (
          <span className="text-xs text-neutral-400">
            このサーバーでは管理権限を持つユーザーのみ予定を編集できます
          </span>
        )}
        {/* 長いサーバー名はスマホで折り返さず 1 行に省略する (#14)。名前だけが縮み、アイコンとボタンは残す */}
        <div className="flex min-w-0 items-center gap-3">
          {guild.avatar_url && (
            // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
            <img
              src={guild.avatar_url}
              alt=""
              className="h-8 w-8 shrink-0 rounded-full"
            />
          )}
          <span className="min-w-0 truncate text-lg font-semibold">
            {guild.name}
          </span>
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
      <EventCalendar guildId={guildId} canEdit={canEdit} />
      <GuildSettingsDialog
        guildId={guildId}
        open={settingsOpen}
        onOpenChange={setSettingsOpen}
      />
    </main>
  );
}
