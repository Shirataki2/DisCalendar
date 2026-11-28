"use client";

import { ManageOnly } from "@/components/guild-settings/manage-only";
import { TrashSettings } from "@/components/trash-settings";

/** サーバー設定の「削除した予定」(ゴミ箱 #159) */
export function GuildTrashSettings({ guildId }: { guildId: string }) {
  return (
    <ManageOnly guildId={guildId}>
      <TrashSettings guildId={guildId} />
    </ManageOnly>
  );
}
