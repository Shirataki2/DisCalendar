"use client";

import { ApiTokenSettings } from "@/components/api-token-settings";
import { IntegrationsByCreator } from "@/components/guild-settings/integrations-by-creator";
import { ManageOnly } from "@/components/guild-settings/manage-only";
import { WebhookSettings } from "@/components/webhook-settings";

/** サーバー設定の「Webhook・API トークン」: 登録したメンバーごとの一覧と、それぞれの登録・管理 */
export function GuildIntegrationSettings({ guildId }: { guildId: string }) {
  return (
    <ManageOnly guildId={guildId}>
      <div className="flex flex-col gap-6">
        <IntegrationsByCreator guildId={guildId} />
        <WebhookSettings guildId={guildId} />
        <ApiTokenSettings guildId={guildId} />
      </div>
    </ManageOnly>
  );
}
