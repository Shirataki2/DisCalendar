import { GuildIntegrationSettings } from "@/components/guild-settings/integration-settings";

export const metadata = { title: "Webhook・API トークン - サーバー設定" };

export default async function Page({
  params,
}: PageProps<"/dashboard/[id]/settings/integrations">) {
  const { id } = await params;
  return <GuildIntegrationSettings key={id} guildId={id} />;
}
