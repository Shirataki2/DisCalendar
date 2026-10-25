import {
  dehydrate,
  HydrationBoundary,
  QueryClient,
} from "@tanstack/react-query";
import type { Metadata } from "next";
import Link from "next/link";
import { notFound, redirect } from "next/navigation";
import { GuildDashboard } from "@/components/guild-dashboard";
import { GuildUnavailable } from "@/components/guild-unavailable";
import { Message } from "@/components/language-provider";
import { ApiError } from "@/lib/api";
import { serverApi } from "@/lib/api/server";
import type { Guild } from "@/lib/api/types";
import { getUserGuilds, guildAccessLabel, guildIconUrl } from "@/lib/discord";
import { loadJoinedGuildIds } from "@/lib/joined-guilds";
import {
  calendarDateParam,
  calendarEventParam,
  loginUrl,
} from "@/lib/login-redirect";
import { queryKeys } from "@/lib/query/keys";

export const metadata: Metadata = {
  title: "カレンダー",
};

type LoadResult = { ok: true; guild: Guild } | { ok: false; error: unknown };

async function loadGuildChoices() {
  const guilds = await getUserGuilds().catch(() => null);
  if (!guilds) return [];
  const joined = await loadJoinedGuildIds(guilds);
  if (!joined.ok) return [];
  return guilds
    .filter((guild) => joined.ids.has(guild.id))
    .map((guild) => ({
      id: guild.id,
      name: guild.name,
      iconUrl: guildIconUrl(guild),
      accessLabel: guildAccessLabel(guild),
    }));
}

// ギルド情報・restricted 設定・自分の権限をまとめて取る。
// メンバーでない / Bot 未参加なら API が 403 を返すので、ここで弾かれる。
// 設定と権限はクライアント側 (サーバー設定ダイアログ) でも更新・再取得するため、
// TanStack Query のキャッシュに入れてブラウザへ hydrate する
async function loadGuild(
  guildId: string,
  queryClient: QueryClient,
): Promise<LoadResult> {
  try {
    const [guild] = await Promise.all([
      serverApi.guilds.get(guildId),
      queryClient.fetchQuery({
        queryKey: queryKeys.guild.config(guildId),
        queryFn: () => serverApi.guilds.config(guildId),
      }),
      queryClient.fetchQuery({
        queryKey: queryKeys.guild.myPermissions(guildId),
        queryFn: () => serverApi.guilds.myPermissions(guildId),
      }),
    ]);
    return { ok: true, guild };
  } catch (error) {
    return { ok: false, error };
  }
}

export default async function GuildCalendarPage({
  params,
  searchParams,
}: PageProps<"/dashboard/[id]">) {
  const { id } = await params;
  const query = await searchParams;
  const initialDate = calendarDateParam(query.date);
  const initialEventId = initialDate
    ? calendarEventParam(query.event)
    : undefined;
  if (!/^\d{1,20}$/.test(id)) {
    notFound();
  }

  // リクエストごとに作る (リクエスト間でキャッシュを共有しない)
  const queryClient = new QueryClient();
  const [result, loadedChoices] = await Promise.all([
    loadGuild(id, queryClient),
    loadGuildChoices(),
  ]);
  if (!result.ok) {
    if (result.error instanceof ApiError && result.error.status === 401) {
      const target = new URLSearchParams();
      if (initialDate) target.set("date", initialDate);
      if (initialEventId) target.set("event", String(initialEventId));
      redirect(loginUrl(`/dashboard/${id}${target.size ? `?${target}` : ""}`));
    }
    return (
      <GuildUnavailable
        error={result.error}
        action={
          <Link
            href="/dashboard"
            className="rounded-full border border-foreground/20 px-5 py-2 text-sm hover:bg-foreground/10"
          >
            <Message message={"サーバー選択に戻る"} />
          </Link>
        }
      />
    );
  }

  const guildChoices = loadedChoices.some((guild) => guild.id === id)
    ? loadedChoices
    : [
        {
          id,
          name: result.guild.name,
          iconUrl: result.guild.avatar_url,
          accessLabel: null,
        },
        ...loadedChoices,
      ];

  return (
    <HydrationBoundary state={dehydrate(queryClient)}>
      <GuildDashboard
        key={result.guild.guild_id}
        guild={result.guild}
        guildChoices={guildChoices}
        initialDate={initialDate}
        initialEventId={initialEventId}
      />
    </HydrationBoundary>
  );
}
