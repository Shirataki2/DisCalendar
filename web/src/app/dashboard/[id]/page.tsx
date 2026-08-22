import type { Metadata } from "next";
import Link from "next/link";
import { notFound, redirect } from "next/navigation";
import { EventCalendar } from "@/components/event-calendar";
import { ApiError } from "@/lib/api";
import { serverApi } from "@/lib/api/server";
import type { Guild, GuildConfig, MyPermissions } from "@/lib/api/types";

export const metadata: Metadata = {
  title: "カレンダー",
};

interface GuildPageData {
  guild: Guild;
  config: GuildConfig;
  permissions: MyPermissions;
}

type LoadResult =
  | { ok: true; data: GuildPageData }
  | { ok: false; error: unknown };

// ギルド情報・restricted 設定・自分の権限をまとめて取る。
// メンバーでない / Bot 未参加なら API が 403 を返すので、ここで弾かれる
async function loadGuild(guildId: string): Promise<LoadResult> {
  try {
    const [guild, config, permissions] = await Promise.all([
      serverApi.guilds.get(guildId),
      serverApi.guilds.config(guildId),
      serverApi.guilds.myPermissions(guildId),
    ]);
    return { ok: true, data: { guild, config, permissions } };
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

  const result = await loadGuild(id);
  if (!result.ok) {
    if (result.error instanceof ApiError && result.error.status === 401) {
      redirect("/login");
    }
    return <GuildUnavailable error={result.error} />;
  }

  const { guild, config, permissions } = result.data;
  // restricted モードでは管理権限を持つユーザーだけが編集できる (API 側でも強制される)
  const canEdit = !config.restricted || permissions.can_manage_server;

  return (
    <main className="flex min-h-0 flex-1 flex-col gap-2 p-4">
      <div className="flex shrink-0 items-center justify-end gap-3">
        {config.restricted && !canEdit && (
          <span className="text-xs text-neutral-400">
            このサーバーでは管理権限を持つユーザーのみ予定を編集できます
          </span>
        )}
        {guild.avatar_url && (
          // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
          <img src={guild.avatar_url} alt="" className="h-8 w-8 rounded-full" />
        )}
        <span className="text-lg font-semibold">{guild.name}</span>
      </div>
      <EventCalendar guildId={id} canEdit={canEdit} />
    </main>
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
