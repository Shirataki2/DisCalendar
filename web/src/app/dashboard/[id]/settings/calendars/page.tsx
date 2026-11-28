import { GuildCalendarSettings } from "@/components/guild-settings/calendar-settings";

export const metadata = { title: "外部カレンダー - サーバー設定" };

export default async function Page({
  params,
}: PageProps<"/dashboard/[id]/settings/calendars">) {
  const { id } = await params;
  return <GuildCalendarSettings key={id} guildId={id} />;
}
