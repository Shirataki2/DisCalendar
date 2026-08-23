"use client";

import { Loader2Icon, RefreshCwIcon } from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { describeApiError } from "@/lib/api";
import {
  ADMIN_SYNC_LIST_LIMIT,
  type AdminGuildSyncCheck,
  type AdminSyncGuild,
} from "@/lib/api/types";
import { useSyncCheck } from "@/lib/query/admin-users";
import { ROUTES } from "@/lib/site";

/**
 * Bot の参加ギルド (Discord API) と `guilds` テーブルの差分検出 (#37)。
 *
 * Bot は参加・退出・更新のイベントで `guilds` を書き換えるので、Bot が止まっている間の出入りは
 * 取りこぼす。ここで両者を突き合わせて、退出済みなのに残っている行や、参加しているのに
 * 登録されていないギルドを見つける。Discord API を全ギルド分辿るのでボタンを押したときだけ実行する
 */
export function AdminSyncCheck() {
  const [started, setStarted] = useState(false);
  const check = useSyncCheck(started);

  return (
    <section
      aria-labelledby="sync-check-heading"
      className="flex flex-col gap-3"
    >
      <div>
        <h2 id="sync-check-heading" className="text-base font-semibold">
          Discord との差分検出
        </h2>
        <p className="mt-1 text-xs text-neutral-400">
          Bot が参加しているギルド (Discord API) と guilds
          テーブルを突き合わせる。 Bot
          の停止中に参加・退出があるとずれるので、その確認に使う。
          差分の一覧は種類ごとに {ADMIN_SYNC_LIST_LIMIT} 件まで表示する
        </p>
      </div>
      <div className="flex flex-wrap items-center gap-3">
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={check.isFetching}
          onClick={() => {
            if (started) {
              void check.refetch();
            } else {
              setStarted(true);
            }
          }}
        >
          {check.isFetching ? (
            <Loader2Icon data-icon="inline-start" className="animate-spin" />
          ) : (
            <RefreshCwIcon data-icon="inline-start" />
          )}
          差分を調べる
        </Button>
        {check.data && !check.isFetching && (
          <span className="text-xs text-neutral-400">
            Discord {check.data.discord_count.toLocaleString()} 件 / DB{" "}
            {check.data.db_count.toLocaleString()} 件
          </span>
        )}
      </div>
      {check.isError && (
        <p
          role="alert"
          className="rounded-md bg-red-900/40 px-3 py-2 text-xs text-red-200"
        >
          {describeApiError(check.error)}
        </p>
      )}
      {check.data && <SyncResult result={check.data} />}
    </section>
  );
}

function SyncResult({ result }: { result: AdminGuildSyncCheck }) {
  const clean =
    result.only_in_db_count === 0 &&
    result.only_in_discord_count === 0 &&
    result.name_mismatch_count === 0;
  if (clean) {
    return (
      <p className="text-sm text-emerald-300">
        差分はありません (Discord と guilds テーブルは一致しています)
      </p>
    );
  }
  return (
    <div className="grid gap-4 lg:grid-cols-3">
      <GuildDiffCard
        title="DB にだけある"
        description="Bot は参加していないのに guilds に行が残っている (退出を取りこぼした)"
        guilds={result.only_in_db}
        total={result.only_in_db_count}
      />
      <GuildDiffCard
        title="Discord にだけある"
        description="Bot は参加しているのに guilds に行が無い (参加を取りこぼした)。/init などで登録される"
        guilds={result.only_in_discord}
        total={result.only_in_discord_count}
      />
      <div className="rounded-lg border border-white/10">
        <h3 className="border-b border-white/10 px-3 py-2 text-sm font-medium">
          名前が違う
          <span className="ml-2 text-xs font-normal text-neutral-400">
            {result.name_mismatch_count.toLocaleString()} 件
          </span>
        </h3>
        <p className="px-3 pt-2 text-xs text-neutral-500">
          Discord 側で改名されたが guilds に反映されていない
        </p>
        {result.name_mismatch.length === 0 ? (
          <p className="px-3 py-3 text-sm text-neutral-500">なし</p>
        ) : (
          <ul className="divide-y divide-white/5">
            {result.name_mismatch.map((item) => (
              <li key={item.guild_id} className="px-3 py-2 text-sm">
                <Link
                  href={`${ROUTES.adminGuilds}/${item.guild_id}`}
                  className="font-mono text-xs text-neutral-300 hover:underline"
                >
                  {item.guild_id}
                </Link>
                <p className="mt-1 text-xs">
                  <span className="text-neutral-400">DB:</span> {item.db_name}
                  <span className="mx-2 text-neutral-500">→</span>
                  <span className="text-neutral-400">Discord:</span>{" "}
                  {item.discord_name}
                </p>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function GuildDiffCard({
  title,
  description,
  guilds,
  total,
}: {
  title: string;
  description: string;
  guilds: AdminSyncGuild[];
  total: number;
}) {
  return (
    <div className="rounded-lg border border-white/10">
      <h3 className="border-b border-white/10 px-3 py-2 text-sm font-medium">
        {title}
        <span className="ml-2 text-xs font-normal text-neutral-400">
          {total.toLocaleString()} 件
        </span>
      </h3>
      <p className="px-3 pt-2 text-xs text-neutral-500">{description}</p>
      {guilds.length === 0 ? (
        <p className="px-3 py-3 text-sm text-neutral-500">なし</p>
      ) : (
        <ul className="divide-y divide-white/5">
          {guilds.map((guild) => (
            <li key={guild.guild_id}>
              <Link
                href={`${ROUTES.adminGuilds}/${guild.guild_id}`}
                className="flex flex-wrap items-center gap-x-2 px-3 py-2 text-sm hover:bg-white/5"
              >
                <span className="truncate">{guild.name ?? "(名前不明)"}</span>
                <span className="ml-auto font-mono text-xs text-neutral-500">
                  {guild.guild_id}
                </span>
              </Link>
            </li>
          ))}
        </ul>
      )}
      {total > guilds.length && (
        <p className="px-3 pb-2 text-xs text-neutral-500">
          ほか {(total - guilds.length).toLocaleString()} 件 (表示は{" "}
          {ADMIN_SYNC_LIST_LIMIT} 件まで)
        </p>
      )}
    </div>
  );
}
