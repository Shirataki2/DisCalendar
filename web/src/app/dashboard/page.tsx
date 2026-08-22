import type { Metadata } from "next";
import Link from "next/link";
import { redirect } from "next/navigation";
import { ApiError } from "@/lib/api";
import { serverApi } from "@/lib/api/server";
import {
  botInviteUrl,
  canInviteBot,
  type DiscordGuild,
  getUserGuilds,
  guildIconUrl,
} from "@/lib/discord";

export const metadata: Metadata = {
  title: "サーバー選択",
};

type JoinedResult =
  | { ok: true; ids: Set<string> }
  | { ok: false; error: unknown };

async function loadJoined(guilds: DiscordGuild[]): Promise<JoinedResult> {
  try {
    const joined = await serverApi.guilds.joined(guilds.map((g) => g.id));
    return { ok: true, ids: new Set(joined.map((g) => g.guild_id)) };
  } catch (error) {
    return { ok: false, error };
  }
}

export default async function DashboardPage() {
  const guilds = await getUserGuilds().catch(() => null);
  if (guilds === null) {
    return (
      <main className="flex-1 overflow-y-auto p-8">
        <h1 className="mb-6 text-xl font-bold">サーバーを選択</h1>
        <div className="text-neutral-300">
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
  const joined = await loadJoined(guilds);
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
        <p className="mb-6 rounded-md bg-red-900/40 px-4 py-2 text-sm text-red-200">
          Bot の参加状況を取得できませんでした。API
          サーバーが起動しているか確認してください。
        </p>
      )}
      {available.length === 0 && joined.ok && (
        <p className="mb-6 text-sm text-neutral-300">
          Bot が参加しているサーバーがありません。下の一覧から Bot
          を招待してください。
        </p>
      )}
      <GuildGrid guilds={available} />
      {invitable.length > 0 && (
        <>
          <h2 className="mt-10 mb-4 text-lg font-semibold text-neutral-300">
            Bot を招待できるサーバー
          </h2>
          <GuildGrid guilds={invitable} invite />
        </>
      )}
    </main>
  );
}

function GuildGrid({
  guilds,
  invite = false,
}: {
  guilds: DiscordGuild[];
  invite?: boolean;
}) {
  if (guilds.length === 0) return null;
  return (
    <ul className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {guilds.map((guild) => (
        <li key={guild.id}>
          <GuildCard guild={guild} invite={invite} />
        </li>
      ))}
    </ul>
  );
}

function GuildCard({
  guild,
  invite,
}: {
  guild: DiscordGuild;
  invite: boolean;
}) {
  const icon = guildIconUrl(guild);
  const className = `flex items-center gap-4 rounded-lg bg-surface p-4 transition-colors hover:bg-white/10 ${
    invite ? "grayscale hover:grayscale-0" : ""
  }`;
  const body = (
    <>
      {icon ? (
        // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
        <img src={icon} alt="" className="h-12 w-12 rounded-full" />
      ) : (
        <span className="flex h-12 w-12 items-center justify-center rounded-full bg-white/10 text-lg font-bold">
          {guild.name.slice(0, 1)}
        </span>
      )}
      <span className="flex-1 font-medium">{guild.name}</span>
      {invite && <span className="text-xs text-neutral-400">招待 ↗</span>}
    </>
  );
  if (invite) {
    // Discord の Bot 追加画面を別タブで開く (旧実装と同じ)。追加後にこの画面を再読込すれば上の一覧に移る
    return (
      <a
        href={botInviteUrl(guild.id)}
        target="_blank"
        rel="noopener noreferrer"
        className={className}
      >
        {body}
      </a>
    );
  }
  return (
    <Link href={`/dashboard/${guild.id}`} className={className}>
      {body}
    </Link>
  );
}
