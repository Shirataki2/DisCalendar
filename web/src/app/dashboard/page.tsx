import type { Metadata } from "next";
import Link from "next/link";
import { getUserGuilds, guildIconUrl } from "@/lib/discord";

export const metadata: Metadata = {
  title: "サーバー選択",
};

export default async function DashboardPage() {
  const guilds = await getUserGuilds().catch(() => null);

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <h1 className="mb-6 text-xl font-bold">サーバーを選択</h1>
      {guilds === null ? (
        <div className="text-neutral-300">
          <p className="mb-2 font-semibold">
            Discordからサーバー一覧を取得できませんでした
          </p>
          <p className="text-sm">
            再ログインするか、時間をおいて再度お試しください。
          </p>
        </div>
      ) : (
        <ul className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {guilds.map((guild) => {
            const icon = guildIconUrl(guild);
            return (
              <li key={guild.id}>
                <Link
                  href={`/dashboard/${guild.id}`}
                  className="flex items-center gap-4 rounded-lg bg-surface p-4 transition-colors hover:bg-white/10"
                >
                  {icon ? (
                    // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
                    <img src={icon} alt="" className="h-12 w-12 rounded-full" />
                  ) : (
                    <span className="flex h-12 w-12 items-center justify-center rounded-full bg-white/10 text-lg font-bold">
                      {guild.name.slice(0, 1)}
                    </span>
                  )}
                  <span className="font-medium">{guild.name}</span>
                </Link>
              </li>
            );
          })}
        </ul>
      )}
    </main>
  );
}
