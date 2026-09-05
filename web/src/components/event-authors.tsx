"use client";

import type { ApiEvent } from "@/lib/api/types";
import { useMemberProfilesQuery } from "@/lib/query/guild";

/** 認証された予定詳細の作成・更新情報。管理コンソールでは ID のみを表示する。 */
export function EventAuthors({
  event,
  active,
  resolveMembers,
}: {
  event: ApiEvent;
  active: boolean;
  resolveMembers: boolean;
}) {
  const ids = [event.created_by, event.updated_by].filter(
    (id): id is string => id !== null,
  );
  const profiles = useMemberProfilesQuery(
    event.guild_id,
    ids,
    active && resolveMembers,
  );
  function person(id: string | null) {
    if (!id) return <span>記録なし</span>;
    if (!resolveMembers) return <span className="break-all">{id}</span>;
    const profile = profiles.data?.find((member) => member.user_id === id);
    if (!profile) {
      return (
        <span>
          {profiles.isError ? "メンバー情報を取得できません" : "読み込み中…"}
        </span>
      );
    }
    return (
      <>
        {profile.avatar_url && (
          // biome-ignore lint/performance/noImgElement: Discord の小さなアバターを直接表示する
          <img
            src={profile.avatar_url}
            alt=""
            className="size-5 shrink-0 rounded-full"
          />
        )}
        <span className="min-w-0 break-words">
          {profile.display_name ?? "退出したメンバー"}
        </span>
      </>
    );
  }
  return (
    <div className="flex flex-col gap-1 text-sm text-muted-foreground">
      <div className="flex items-center gap-1.5">
        <span className="shrink-0">作成:</span>
        {person(event.created_by)}
      </div>
      {event.updated_by && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="shrink-0">最終更新:</span>
          {person(event.updated_by)}
          {event.updated_at && (
            <time dateTime={`${event.updated_at}+09:00`}>
              ({formatUpdatedAt(event.updated_at)})
            </time>
          )}
        </div>
      )}
    </div>
  );
}

/** API の JST naive をブラウザのタイムゾーンによらず表示する。 */
function formatUpdatedAt(value: string) {
  const [date, time] = value.split("T");
  const [, month, day] = date.split("-");
  return `${Number(month)}/${Number(day)} ${time.slice(0, 5)}`;
}
