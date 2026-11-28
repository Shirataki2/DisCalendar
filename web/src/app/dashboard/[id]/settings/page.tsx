import { GuildGeneralSettings } from "@/components/guild-settings/general-settings";

export const metadata = { title: "サーバー設定" };

export default async function Page({
  params,
}: PageProps<"/dashboard/[id]/settings">) {
  const { id } = await params;
  return <GuildGeneralSettings key={id} guildId={id} />;
}
