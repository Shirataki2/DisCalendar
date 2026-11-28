import {
  dehydrate,
  HydrationBoundary,
  QueryClient,
} from "@tanstack/react-query";
import Link from "next/link";
import { notFound, redirect } from "next/navigation";
import { GuildSettingsShell } from "@/components/guild-settings/settings-shell";
import { GuildUnavailable } from "@/components/guild-unavailable";
import { Message } from "@/components/language-provider";
import { ApiError } from "@/lib/api";
import { serverApi } from "@/lib/api/server";
import { loginUrl } from "@/lib/login-redirect";
import { queryKeys } from "@/lib/query/keys";

/**
 * サーバー設定ページ (#440。節ごとに `/settings`, `/settings/calendars` など) の共通レイアウト。
 * ギルド情報・設定・自分の権限を取り、TanStack Query のキャッシュに入れてブラウザへ渡す
 * (カレンダー画面と同じキー。保存した設定はそのままカレンダーの編集可否に反映される)。
 * メンバーでない / Bot 未参加なら API が 403 を返すので、ここで弾かれる
 */
export default async function GuildSettingsLayout({
  children,
  params,
}: LayoutProps<"/dashboard/[id]/settings">) {
  const { id } = await params;
  if (!/^\d{1,20}$/.test(id)) notFound();

  // リクエストごとに作る (リクエスト間でキャッシュを共有しない)
  const queryClient = new QueryClient();
  let guildName: string;
  try {
    const [guild] = await Promise.all([
      serverApi.guilds.get(id),
      queryClient.fetchQuery({
        queryKey: queryKeys.guild.config(id),
        queryFn: () => serverApi.guilds.config(id),
      }),
      queryClient.fetchQuery({
        queryKey: queryKeys.guild.myPermissions(id),
        queryFn: () => serverApi.guilds.myPermissions(id),
      }),
    ]);
    guildName = guild.name ?? id;
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      redirect(loginUrl(`/dashboard/${id}/settings`));
    }
    return (
      <GuildUnavailable
        error={error}
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

  return (
    <HydrationBoundary state={dehydrate(queryClient)}>
      <GuildSettingsShell key={id} guildId={id} guildName={guildName}>
        {children}
      </GuildSettingsShell>
    </HydrationBoundary>
  );
}
