"use client";

import { ArrowLeftIcon } from "lucide-react";
import Link from "next/link";
import { useId, useState } from "react";
import { EventCalendar } from "@/components/event-calendar";
import { Checkbox } from "@/components/ui/checkbox";
import { describeApiError } from "@/lib/api";
import {
  useAdminGuildQuery,
  useUpdateAdminGuildConfig,
} from "@/lib/query/admin";
import { adminEventsSource } from "@/lib/query/events";
import { ROUTES } from "@/lib/site";

interface Props {
  guildId: string;
}

/**
 * 管理コンソールのギルド詳細 (#35)。ギルド情報 + restricted の切替 + 予定のカレンダー。
 * カレンダーは通常画面 (`/dashboard/[id]`) と同じ `EventCalendar` を admin 用 API に向けて使う。
 * 管理者は restricted に関係なく編集できる (api 側も `AdminUser` だけを見る)
 */
export function AdminGuildView({ guildId }: Props) {
  const guildQuery = useAdminGuildQuery(guildId);
  const updateConfig = useUpdateAdminGuildConfig(guildId);
  const [configError, setConfigError] = useState<string | null>(null);
  const restrictedId = useId();
  const guild = guildQuery.data;

  const toggleRestricted = async (restricted: boolean) => {
    setConfigError(null);
    try {
      await updateConfig.mutateAsync(restricted);
    } catch (error) {
      setConfigError(describeApiError(error));
    }
  };

  return (
    <main className="flex min-h-0 flex-1 flex-col gap-3 p-4">
      <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2">
        <Link
          href={ROUTES.adminGuilds}
          className="flex items-center gap-1 text-xs text-neutral-400 hover:text-white"
        >
          <ArrowLeftIcon className="size-3.5" />
          ギルド一覧
        </Link>
        {guild?.avatar_url && (
          // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
          <img src={guild.avatar_url} alt="" className="h-8 w-8 rounded-full" />
        )}
        <h1 className="text-lg font-semibold">{guild?.name ?? guildId}</h1>
        {guild && (
          <dl className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-neutral-400">
            <div className="flex gap-1">
              <dt>ID</dt>
              <dd className="font-mono text-neutral-200">{guild.guild_id}</dd>
            </div>
            <div className="flex gap-1">
              <dt>Bot</dt>
              <dd>
                {guild.bot_joined === null ? (
                  <span title="Discord API に問い合わせられませんでした">
                    不明
                  </span>
                ) : guild.bot_joined ? (
                  <span className="text-emerald-300">参加中</span>
                ) : (
                  <span className="text-red-300">未参加 (退出済み)</span>
                )}
              </dd>
            </div>
            <div className="flex gap-1">
              <dt>通知チャンネル</dt>
              <dd className="font-mono text-neutral-200">
                {guild.channel_id ?? "未設定"}
              </dd>
            </div>
            <div className="flex gap-1">
              <dt>予定数</dt>
              <dd className="tabular-nums text-neutral-200">
                {guild.event_count}
              </dd>
            </div>
          </dl>
        )}
        <label
          htmlFor={restrictedId}
          className="ml-auto flex items-center gap-2 text-xs text-neutral-300"
        >
          <Checkbox
            id={restrictedId}
            checked={guild?.restricted ?? false}
            disabled={!guild || updateConfig.isPending}
            onCheckedChange={(checked) => void toggleRestricted(checked)}
          />
          restricted (管理権限を持つユーザーのみ編集可)
        </label>
        {guildQuery.isError && (
          <span className="rounded-md bg-red-900/40 px-3 py-1.5 text-sm text-red-200">
            ギルド情報を取得できませんでした:{" "}
            {describeApiError(guildQuery.error)}
          </span>
        )}
        {configError && (
          <span className="flex items-center gap-2 rounded-md bg-red-900/40 px-3 py-1.5 text-sm text-red-200">
            設定を変更できませんでした: {configError}
            <button
              type="button"
              onClick={() => setConfigError(null)}
              className="underline hover:text-white"
            >
              閉じる
            </button>
          </span>
        )}
      </div>
      <p className="shrink-0 text-xs text-amber-300/80">
        管理者として操作しています。予定の作成・編集・削除と restricted
        の切替は監査ログに記録されます
      </p>
      <EventCalendar
        guildId={guildId}
        canEdit
        eventsSource={adminEventsSource}
      />
    </main>
  );
}
