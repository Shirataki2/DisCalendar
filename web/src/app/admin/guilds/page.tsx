import { SearchIcon } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { AdminPagination } from "@/components/admin-pagination";
import { AdminSyncCheck } from "@/components/admin-sync-check";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { firstParam, lastPageOf, listHref, parsePage } from "@/lib/admin-list";
import { serverApi } from "@/lib/api/server";
import { ADMIN_GUILDS_MAX_PAGE, type AdminGuild } from "@/lib/api/types";
import { ROUTES } from "@/lib/site";

export const metadata: Metadata = {
  title: "ギルド一覧 | 管理コンソール",
};

/**
 * 全ギルドの一覧・検索 (#35) と、Discord 側との差分検出 (#37)。
 * 検索とページングは URL (q / page) に持たせて RSC で取得する
 * (件数が少なく、共有・再読込しやすい方が運用には向く)
 */
export default async function AdminGuildsPage({
  searchParams,
}: PageProps<"/admin/guilds">) {
  const params = await searchParams;
  const q = firstParam(params.q).trim();
  const page = parsePage(firstParam(params.page), ADMIN_GUILDS_MAX_PAGE);
  const result = await serverApi.admin.guilds.list(q, page);
  const lastPage = lastPageOf(result.total, result.page_size);

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-xl font-bold">ギルド一覧</h1>
          <p className="mt-1 text-sm text-neutral-400">
            Bot が参加した (ことのある)
            全ギルド。退出済みでも予定や設定が残っていれば載る。
            {result.total} 件{q && <> (「{q}」で検索)</>}
          </p>
        </div>
        <form
          action={ROUTES.adminGuilds}
          method="get"
          className="flex items-center gap-2"
        >
          <Input
            type="search"
            name="q"
            defaultValue={q}
            placeholder="ギルド ID または名前"
            aria-label="ギルドを検索"
            className="w-64"
          />
          <Button type="submit" variant="outline" size="sm">
            <SearchIcon data-icon="inline-start" />
            検索
          </Button>
        </form>
      </div>

      <div className="overflow-x-auto rounded-lg border border-white/10">
        <table className="w-full text-sm">
          <thead className="bg-white/5 text-left text-xs text-neutral-400">
            <tr>
              <th className="px-3 py-2 font-medium">ギルド</th>
              <th className="px-3 py-2 font-medium">ギルド ID</th>
              <th className="px-3 py-2 font-medium">restricted</th>
              <th className="px-3 py-2 font-medium">通知チャンネル</th>
              <th className="px-3 py-2 text-right font-medium">予定数</th>
            </tr>
          </thead>
          <tbody>
            {result.items.length === 0 ? (
              <tr>
                <td
                  colSpan={5}
                  className="px-3 py-8 text-center text-neutral-500"
                >
                  該当するギルドはありません
                </td>
              </tr>
            ) : (
              result.items.map((guild) => (
                <GuildRow key={guild.guild_id} guild={guild} />
              ))
            )}
          </tbody>
        </table>
      </div>

      <AdminPagination
        page={page}
        lastPage={lastPage}
        href={(next) => listHref(ROUTES.adminGuilds, { q }, next)}
      />

      <div className="mt-10">
        <AdminSyncCheck />
      </div>
    </main>
  );
}

function GuildRow({ guild }: { guild: AdminGuild }) {
  return (
    <tr className="border-t border-white/10 hover:bg-white/5">
      <td className="px-3 py-2">
        <Link
          href={`${ROUTES.adminGuilds}/${guild.guild_id}`}
          className="flex items-center gap-2 font-medium hover:underline"
        >
          {guild.avatar_url ? (
            // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
            <img
              src={guild.avatar_url}
              alt=""
              className="h-6 w-6 rounded-full"
            />
          ) : (
            <span
              aria-hidden="true"
              className="inline-flex h-6 w-6 items-center justify-center rounded-full bg-white/10 text-xs"
            >
              {guild.name?.slice(0, 1) ?? "?"}
            </span>
          )}
          {guild.name ?? <span className="text-neutral-400">(名前不明)</span>}
          {!guild.registered && (
            <span className="rounded bg-red-500/20 px-1.5 py-0.5 text-[10px] text-red-300">
              退出済み
            </span>
          )}
        </Link>
      </td>
      <td className="px-3 py-2 font-mono text-xs text-neutral-300">
        {guild.guild_id}
      </td>
      <td className="px-3 py-2">
        {guild.restricted ? (
          <span className="rounded bg-amber-500/20 px-1.5 py-0.5 text-xs text-amber-300">
            ON
          </span>
        ) : (
          <span className="text-xs text-neutral-500">OFF</span>
        )}
      </td>
      <td className="px-3 py-2 font-mono text-xs text-neutral-300">
        {guild.channel_id ?? <span className="text-neutral-500">未設定</span>}
      </td>
      <td className="px-3 py-2 text-right tabular-nums">{guild.event_count}</td>
    </tr>
  );
}
