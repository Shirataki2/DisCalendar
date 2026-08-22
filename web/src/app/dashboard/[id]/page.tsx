import {
  dehydrate,
  HydrationBoundary,
  QueryClient,
} from "@tanstack/react-query";
import type { Metadata } from "next";
import Link from "next/link";
import { notFound, redirect } from "next/navigation";
import { GuildDashboard } from "@/components/guild-dashboard";
import { ApiError } from "@/lib/api";
import { serverApi } from "@/lib/api/server";
import type { Guild } from "@/lib/api/types";
import { queryKeys } from "@/lib/query/keys";

export const metadata: Metadata = {
  title: "カレンダー",
};

type LoadResult = { ok: true; guild: Guild } | { ok: false; error: unknown };

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
}: PageProps<"/dashboard/[id]">) {
  const { id } = await params;
  if (!/^\d{1,20}$/.test(id)) {
    notFound();
  }

  // リクエストごとに作る (リクエスト間でキャッシュを共有しない)
  const queryClient = new QueryClient();
  const result = await loadGuild(id, queryClient);
  if (!result.ok) {
    if (result.error instanceof ApiError && result.error.status === 401) {
      redirect("/login");
    }
    return <GuildUnavailable error={result.error} />;
  }

  return (
    <HydrationBoundary state={dehydrate(queryClient)}>
      <GuildDashboard guild={result.guild} />
    </HydrationBoundary>
  );
}

function GuildUnavailable({ error }: { error: unknown }) {
  const discordTrouble =
    error instanceof ApiError &&
    (error.kind === "discord_error" || error.kind === "rate_limited");
  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-4 p-8 text-center">
      <h1 className="text-xl font-bold">サーバーデータの取得に失敗しました</h1>
      {discordTrouble ? (
        <p className="text-sm text-neutral-300">
          Discord との通信に失敗しました。時間をおいて再度お試しください。
        </p>
      ) : (
        <div className="text-sm text-neutral-300">
          <p className="mb-2">以下の事項をご確認ください</p>
          <p>･ BOTがサーバーに導入されているか</p>
          <p>･ あなた自身がBOTを導入したサーバーに参加しているか</p>
        </div>
      )}
      <Link
        href="/dashboard"
        className="rounded-full border border-white/20 px-5 py-2 text-sm hover:bg-white/10"
      >
        サーバー選択に戻る
      </Link>
    </main>
  );
}
