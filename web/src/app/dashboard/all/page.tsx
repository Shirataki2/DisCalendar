import type { Metadata } from "next";
import Link from "next/link";
import { redirect } from "next/navigation";
import type { ReactNode } from "react";
import { JoinedEventsCalendar } from "@/components/joined-events-calendar";
import { ApiError } from "@/lib/api";
import { getUserGuilds, guildIconUrl } from "@/lib/discord";
import { loadJoinedGuildIds } from "@/lib/joined-guilds";
import { ROUTES } from "@/lib/site";

export const metadata: Metadata = {
  title: "すべての予定",
};

/**
 * 参加している全サーバーの予定をまとめて見る画面 (#98)。
 * サーバー選択画面と同じ手順で Bot 参加済みのサーバーを求め、その一覧を閲覧専用のカレンダーに渡す
 * (予定の取得と各サーバーのメンバー確認は api の `/events/@me` が行う)
 */
export default async function AllEventsPage() {
  const guilds = await getUserGuilds().catch(() => null);
  if (guilds === null) {
    return (
      <Unavailable title="Discordからサーバー一覧を取得できませんでした">
        再ログインするか、時間をおいて再度お試しください。
      </Unavailable>
    );
  }

  // サーバー選択画面と違い、Bot の参加状況が分からないと何も出せないのでフォールバックはしない
  const joined = await loadJoinedGuildIds(guilds);
  if (!joined.ok) {
    if (joined.error instanceof ApiError && joined.error.status === 401) {
      redirect("/login");
    }
    return (
      <Unavailable title="Bot の参加状況を取得できませんでした">
        時間をおいて再度お試しください。
      </Unavailable>
    );
  }
  // 並びは Discord の一覧のまま (サーバー選択画面と同じ順。凡例の色もこの順で決まる)
  const available = guilds.filter((guild) => joined.ids.has(guild.id));
  if (available.length === 0) {
    return (
      <Unavailable title="Bot が参加しているサーバーがありません">
        サーバー一覧から Bot を招待すると、そのサーバーの予定がここにまとめて表示されます。
      </Unavailable>
    );
  }

  return (
    <main className="flex min-h-0 flex-1 flex-col gap-2 p-4">
      <div className="flex shrink-0 flex-wrap items-baseline gap-x-3 gap-y-1">
        <h1 className="text-lg font-semibold">すべての予定</h1>
        <span className="text-xs text-muted-foreground">
          {available.length}{" "}
          サーバーの予定をまとめて表示しています。予定の作成・編集は各サーバーのカレンダーで行えます
        </span>
      </div>
      <JoinedEventsCalendar
        guilds={available.map((guild) => ({
          id: guild.id,
          name: guild.name,
          iconUrl: guildIconUrl(guild),
        }))}
      />
    </main>
  );
}

function Unavailable({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-4 p-8 text-center">
      <h1 className="text-xl font-bold">{title}</h1>
      <p className="text-sm text-muted-foreground">{children}</p>
      <Link
        href={ROUTES.dashboard}
        className="rounded-full border border-foreground/20 px-5 py-2 text-sm hover:bg-foreground/10"
      >
        サーバー一覧へ
      </Link>
    </main>
  );
}
