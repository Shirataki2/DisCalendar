"use client";

import { ExternalCalendarSettings } from "@/components/external-calendar-settings";
import { FeedSettings } from "@/components/guild-settings/feed-settings";
import { Separator } from "@/components/ui/separator";
import { useCanManageServer } from "@/lib/query/guild";

/** サーバー設定の「外部カレンダー」: このサーバーの予定の配信 (iCal フィード #95) と、外部カレンダーの重ね表示 */
export function GuildCalendarSettings({ guildId }: { guildId: string }) {
  const canManage = useCanManageServer(guildId) ?? false;
  return (
    <div className="flex flex-col gap-6">
      <FeedSettings guildId={guildId} canManage={canManage} />
      <Separator />
      <ExternalCalendarSettings guildId={guildId} canManage={canManage} />
    </div>
  );
}
