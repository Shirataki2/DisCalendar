import { CalendarDaysIcon } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { redirect } from "next/navigation";
import {
  GuildCardBody,
  GuildGrid,
  guildCardClassName,
} from "@/components/guild-card";
import { InviteGuildGrid } from "@/components/invite-guild-grid";
import { ApiError } from "@/lib/api";
import {
  botInviteUrl,
  canInviteBot,
  type DiscordGuild,
  getUserGuilds,
  guildIconUrl,
} from "@/lib/discord";
import { loadJoinedGuildIds } from "@/lib/joined-guilds";
import { ROUTES } from "@/lib/site";

export const metadata: Metadata = {
  title: "サーバー選択",
};

export default async function DashboardPage() {
  const guilds = await getUserGuilds().catch(() => null);
  if (guilds === null) {
    return (
      <main className="flex-1 overflow-y-auto p-8">
        <h1 className="mb-6 text-xl font-bold">サーバーを選択</h1>
        <div className="text-muted-foreground">
          <p className="mb-2 font-semibold">
            Discordからサーバー一覧を取得できませんでした
          </p>
          <p className="text-sm">
            再ログインするか、時間をおいて再度お試しください。
          </p>
        </div>
      </main>
    );
  }

  // Bot が参加しているサーバー (カレンダーが使える) と、管理権限があり Bot を招待できるサーバーに分ける
  const joined = await loadJoinedGuildIds(guilds);
  if (
    !joined.ok &&
    joined.error instanceof ApiError &&
    joined.error.status === 401
  ) {
    redirect("/login");
  }
  const joinedIds = joined.ok ? joined.ids : new Set(guilds.map((g) => g.id));
  const available = guilds.filter((g) => joinedIds.has(g.id));
  const invitable = joined.ok
    ? guilds.filter((g) => !joinedIds.has(g.id) && canInviteBot(g))
    : [];

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <h1 className="mb-6 text-xl font-bold">サーバーを選択</h1>
      {!joined.ok && (
        <p className="mb-6 rounded-md bg-destructive/10 px-4 py-2 text-sm text-destructive">
          Bot の参加状況を取得できませんでした。API
          サーバーが起動しているか確認してください。
        </p>
      )}
      {available.length === 0 && joined.ok && (
        <p className="mb-6 text-sm text-muted-foreground">
          Bot が参加しているサーバーがありません。下の一覧から Bot
          を招待してください。
        </p>
      )}
      {/* 横断カレンダー (#98)。参加状況が取れているときだけ出す (取れていないと全サーバーが「参加済み」に見えている) */}
      {joined.ok && available.length > 0 && (
        <AllEventsCard count={available.length} />
      )}
      <JoinedGuildGrid guilds={available} />
      {invitable.length > 0 && (
        <>
          <h2 className="mt-10 mb-4 text-lg font-semibold text-muted-foreground">
            Bot を招待できるサーバー
          </h2>
          {/* 招待後に戻ってきたら参加状況を見て自動で移動するので、ここだけクライアント側で描く */}
          <InviteGuildGrid
            guilds={invitable.map((guild) => ({
              id: guild.id,
              name: guild.name,
              iconUrl: guildIconUrl(guild),
              inviteUrl: botInviteUrl(guild.id),
            }))}
          />
        </>
      )}
    </main>
  );
}

/**
 * 「すべての予定」への入口。サーバーのカードと同じ見た目で一覧の上に 1 枚だけ置く。
 * サーバー名はカードに書かない (サーバー名でカードを探す導線・テストと混ざらないように)
 */
function AllEventsCard({ count }: { count: number }) {
  return (
    <Link href={ROUTES.dashboardAll} className={`${guildCardClassName()} mb-4`}>
      <span className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-indigo-500/15 text-indigo-700 dark:text-indigo-300">
        <CalendarDaysIcon className="size-6" aria-hidden />
      </span>
      <span className="flex-1 font-medium">すべての予定</span>
      <span className="shrink-0 text-xs text-muted-foreground">
        {count} サーバーの予定をまとめて表示
      </span>
    </Link>
  );
}

function JoinedGuildGrid({ guilds }: { guilds: DiscordGuild[] }) {
  if (guilds.length === 0) return null;
  return (
    <GuildGrid>
      {guilds.map((guild) => (
        <li key={guild.id}>
          <Link
            href={`/dashboard/${guild.id}`}
            className={guildCardClassName()}
          >
            <GuildCardBody name={guild.name} iconUrl={guildIconUrl(guild)} />
          </Link>
        </li>
      ))}
    </GuildGrid>
  );
}
