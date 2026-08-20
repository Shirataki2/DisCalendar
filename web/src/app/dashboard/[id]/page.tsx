import type { Metadata } from "next";
import { EventCalendar } from "@/components/event-calendar";
import { getUserGuilds } from "@/lib/discord";

export const metadata: Metadata = {
  title: "カレンダー",
};

export default async function GuildCalendarPage({
  params,
}: PageProps<"/dashboard/[id]">) {
  const { id } = await params;
  const guilds = await getUserGuilds().catch(() => []);
  const guild = guilds.find((g) => g.id === id);

  return (
    <main className="flex min-h-0 flex-1 flex-col gap-2 p-4">
      {guild && (
        <div className="shrink-0 text-right text-lg font-semibold">
          {guild.name}
        </div>
      )}
      <EventCalendar />
    </main>
  );
}
