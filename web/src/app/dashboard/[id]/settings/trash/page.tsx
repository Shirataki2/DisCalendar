import { GuildTrashSettings } from "@/components/guild-settings/trash-section";

export const metadata = { title: "削除した予定 - サーバー設定" };

export default async function Page({
  params,
}: PageProps<"/dashboard/[id]/settings/trash">) {
  const { id } = await params;
  return <GuildTrashSettings key={id} guildId={id} />;
}
