"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { EventDescription } from "@/components/event-description";
import {
  type EventDialogState,
  EventFormDialog,
} from "@/components/event-form-dialog";
import { PollFormDialog } from "@/components/poll-form-dialog";
import { Button } from "@/components/ui/button";
import { readCalendarSettings } from "@/hooks/use-calendar-settings";
import { api, describeApiError } from "@/lib/api";
import type {
  MemberProfile,
  PollAnnouncement,
  PollAnswer,
  PollDetail,
  PollOption,
} from "@/lib/api/types";
import { parseApiDateTime } from "@/lib/calendar-events";
import { newEventFormValues } from "@/lib/event-form";
import {
  dashboardEventsSource,
  invalidateEvents,
  refetchPermissionsOnBotError,
} from "@/lib/query/events";
import {
  canEditEvents,
  useGuildConfigQuery,
  useMyPermissionsQuery,
  useRefreshMyPermissions,
} from "@/lib/query/guild";
import { queryKeys } from "@/lib/query/keys";

const statusLabels = {
  open: "進行中",
  closed: "締切済み",
  confirmed: "確定済み",
};
const answers: { value: PollAnswer; label: string }[] = [
  { value: "yes", label: "○ 参加できる" },
  { value: "maybe", label: "△ 未定" },
  { value: "no", label: "× 参加できない" },
];
export function PollList({ guildId }: { guildId: string }) {
  const router = useRouter();
  const client = useQueryClient();
  const permissions = useMyPermissionsQuery(guildId);
  const canEdit = canEditEvents(permissions.data);
  const [creating, setCreating] = useState(false);
  const query = useQuery({
    queryKey: queryKeys.polls.list(guildId),
    queryFn: ({ signal }) => api.polls.list(guildId, signal),
    refetchInterval: 30_000,
  });
  return (
    <main className="min-h-0 flex-1 space-y-6 overflow-y-auto p-4 sm:p-6">
      <Link
        href={`/dashboard/${guildId}`}
        className="inline-flex min-h-11 items-center underline"
      >
        カレンダーに戻る
      </Link>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-bold">日程調整</h1>
        <Button
          className="min-h-11"
          disabled={!canEdit}
          onClick={() => setCreating(true)}
        >
          日程調整を作成
        </Button>
      </div>
      <p className="text-sm text-muted-foreground">
        候補日への回答を集め、そのまま予定にできます。最新100件を表示します。
      </p>
      {query.isPending && <p role="status">読み込み中…</p>}
      {query.isError && (
        <p role="alert">
          {describeApiError(query.error)}{" "}
          <Button variant="outline" onClick={() => query.refetch()}>
            再試行
          </Button>
        </p>
      )}
      {query.data &&
        (["open", "closed", "confirmed"] as const).map((status) => (
          <section key={status} className="space-y-3">
            <h2 className="font-semibold">{statusLabels[status]}</h2>
            <ul className="space-y-2">
              {query.data
                .filter((p) => p.status === status)
                .map((p) => (
                  <li key={p.id}>
                    <Link
                      className="block rounded-lg border p-4 hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring"
                      href={`/dashboard/${guildId}/polls/${p.id}`}
                    >
                      <span className="block break-words font-medium">
                        {p.title}
                      </span>
                      <span className="text-sm text-muted-foreground">
                        締切: {p.deadline ? dateTime(p.deadline) : "なし"}
                      </span>
                    </Link>
                  </li>
                ))}
            </ul>
            {!query.data.some((p) => p.status === status) && (
              <p className="text-sm text-muted-foreground">
                日程調整はありません。
              </p>
            )}
          </section>
        ))}
      {creating && (
        <PollFormDialog
          poll={null}
          onClose={() => setCreating(false)}
          onSave={async (input) => {
            const result = await api.polls.create(guildId, input);
            await client.invalidateQueries({
              queryKey: queryKeys.polls.all(guildId),
            });
            router.push(
              `/dashboard/${guildId}/polls/${result.poll.id}?announcement=${result.announcement}`,
            );
          }}
        />
      )}
    </main>
  );
}
export function PollPage({
  guildId,
  pollId,
  initialAnnouncement,
}: {
  guildId: string;
  pollId: number;
  initialAnnouncement?: PollAnnouncement;
}) {
  const router = useRouter();
  const client = useQueryClient();
  const config = useGuildConfigQuery(guildId);
  const guild = useQuery({
    queryKey: queryKeys.guild.detail(guildId),
    queryFn: () => api.guilds.get(guildId),
  });
  const permissions = useMyPermissionsQuery(guildId);
  const refresh = useRefreshMyPermissions(guildId);
  const canEdit = canEditEvents(permissions.data);
  const [editing, setEditing] = useState(false);
  const [confirmation, setConfirmation] = useState<{
    option: PollOption;
    version: number;
    state: EventDialogState;
  } | null>(null);
  const [announcement, setAnnouncement] = useState(initialAnnouncement);
  useEffect(() => {
    if (initialAnnouncement)
      window.history.replaceState(
        null,
        "",
        `/dashboard/${guildId}/polls/${pollId}`,
      );
  }, [guildId, pollId, initialAnnouncement]);
  const query = useQuery({
    queryKey: queryKeys.polls.detail(guildId, pollId),
    queryFn: ({ signal }) => api.polls.detail(guildId, pollId, signal),
    refetchInterval: 15_000,
  });
  const poll = query.data;
  const changed = () =>
    client.invalidateQueries({ queryKey: queryKeys.polls.all(guildId) });
  const action = useMutation({
    mutationFn: async (fn: () => Promise<void>) => fn(),
    onSettled: changed,
  });
  const expired =
    !!poll?.deadline && Date.parse(`${poll.deadline}+09:00`) <= Date.now();
  const open = poll?.status === "open" && !expired;
  function confirm(option: PollOption, poll: PollDetail) {
    if (!config.isSuccess || !guild.isSuccess) return;
    const start = parseApiDateTime(`${option.start_at.slice(0, 10)}T00:00:00`),
      end = parseApiDateTime(`${option.end_at.slice(0, 10)}T00:00:00`);
    setConfirmation({
      option,
      version: poll.version,
      state: {
        mode: "create",
        values: {
          ...newEventFormValues(
            start,
            end,
            false,
            config.data?.default_notifications,
            readCalendarSettings(),
          ),
          isAllDay: option.is_all_day,
          startTime: option.start_at.slice(11, 16),
          endTime: option.end_at.slice(11, 16),
          name: poll.title,
          description: poll.description ?? "",
        },
      },
    });
  }
  return (
    <main className="min-h-0 min-w-0 flex-1 space-y-5 overflow-y-auto p-4 sm:p-6">
      <Link
        href={`/dashboard/${guildId}/polls`}
        className="inline-flex min-h-11 items-center underline"
      >
        日程調整の一覧に戻る
      </Link>
      {query.isPending && <p role="status">読み込み中…</p>}
      {query.isError && (
        <p role="alert">
          {describeApiError(query.error)}{" "}
          <Button variant="outline" onClick={() => query.refetch()}>
            再試行
          </Button>
        </p>
      )}
      {poll && (
        <>
          <div className="space-y-2">
            <p className="text-sm text-muted-foreground">
              {
                statusLabels[
                  expired && poll.status === "open" ? "closed" : poll.status
                ]
              }
              ・日本時間
            </p>
            <h1 className="break-words text-2xl font-bold">{poll.title}</h1>
            <p>締切: {poll.deadline ? dateTime(poll.deadline) : "なし"}</p>
            {poll.description && (
              <EventDescription>{poll.description}</EventDescription>
            )}
          </div>
          <Announcement status={announcement} />
          {poll.status === "confirmed" && (
            <p className="rounded-lg border p-3">
              日程が確定しました。
              <Link href={`/dashboard/${guildId}`} className="ml-2 underline">
                カレンダーを見る
              </Link>
            </p>
          )}
          {!open && poll.status !== "confirmed" && (
            <p role="status">
              投票は締め切られています。集計から予定を確定できます。
            </p>
          )}
          <section className="space-y-3" aria-label="候補と自分の回答">
            <h2 className="font-semibold">候補と自分の回答</h2>
            {poll.options.map((option, index) => {
              const mine = poll.votes.find(
                (v) =>
                  v.option_id === option.id &&
                  v.user_id === poll.current_user_id,
              )?.answer;
              return (
                <div
                  key={option.id}
                  className="space-y-3 rounded-lg border p-4"
                >
                  <h3 className="font-medium">
                    候補 {index + 1}: {optionLabel(option)}
                  </h3>
                  <p className="text-sm" aria-live="polite">
                    ○ {option.yes}人 / △ {option.maybe}人 / × {option.no}人
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {answers.map((answer) => (
                      <Button
                        key={answer.value}
                        type="button"
                        className="min-h-11"
                        variant={mine === answer.value ? "default" : "outline"}
                        aria-pressed={mine === answer.value}
                        disabled={!open || action.isPending}
                        onClick={() =>
                          action.mutate(() =>
                            api.polls.vote(
                              guildId,
                              pollId,
                              option.id,
                              answer.value,
                            ),
                          )
                        }
                      >
                        {answer.label}
                      </Button>
                    ))}
                    {canEdit && poll.status !== "confirmed" && (
                      <Button
                        variant="secondary"
                        className="min-h-11"
                        disabled={
                          action.isPending ||
                          !config.isSuccess ||
                          !guild.isSuccess
                        }
                        onClick={() => confirm(option, poll)}
                      >
                        この候補で確定
                      </Button>
                    )}
                  </div>
                </div>
              );
            })}
          </section>
          {canEdit && (config.isError || guild.isError) && (
            <p role="alert">
              保存先・通知設定を取得できませんでした。
              <Button
                variant="outline"
                onClick={() => {
                  void config.refetch();
                  void guild.refetch();
                }}
              >
                再取得
              </Button>
            </p>
          )}
          <VoteTable poll={poll} />
          {action.isError && (
            <p role="alert" className="text-destructive">
              {describeApiError(action.error)}
            </p>
          )}
          {canEdit && (
            <div className="flex flex-wrap gap-2">
              {open && (
                <>
                  <Button
                    variant="outline"
                    className="min-h-11"
                    disabled={action.isPending}
                    onClick={() => setEditing(true)}
                  >
                    候補を編集
                  </Button>
                  <Button
                    variant="outline"
                    className="min-h-11"
                    disabled={action.isPending}
                    onClick={() => {
                      if (window.confirm("投票を締め切りますか？"))
                        action.mutate(() =>
                          api.polls.close(guildId, pollId, poll.version),
                        );
                    }}
                  >
                    投票を締め切る
                  </Button>
                </>
              )}
              <Button
                variant="destructive"
                className="min-h-11"
                disabled={action.isPending}
                onClick={() => {
                  if (
                    window.confirm(
                      "日程調整と回答を削除しますか？ 確定済みの予定は残ります。",
                    )
                  )
                    action.mutate(async () => {
                      await api.polls.remove(guildId, pollId);
                      router.push(`/dashboard/${guildId}/polls`);
                    });
                }}
              >
                日程調整を削除
              </Button>
            </div>
          )}
          {editing && (
            <PollFormDialog
              poll={poll}
              onClose={() => setEditing(false)}
              onSave={async (input) => {
                await api.polls.update(guildId, pollId, input);
                setEditing(false);
                await changed();
              }}
            />
          )}
          <EventFormDialog
            fixedSchedule
            guildName={guild.data?.name ?? ""}
            mentionGuildId={guildId}
            state={confirmation?.state ?? null}
            onClose={() => setConfirmation(null)}
            onDelete={() => {}}
            guidance={
              <p className="rounded border p-3 text-sm">
                選んだ候補で予定を作成します。色・通知・Discordイベント連携を設定できます。
              </p>
            }
            discordSync={{
              botCreateEvents: permissions.data?.bot_create_events ?? false,
              canCreateEvents: permissions.data?.create_events ?? false,
              onRefresh: () => refresh.mutateAsync(),
            }}
            onSubmit={async (input) => {
              if (!confirmation) throw new Error("候補を選択してください");
              const result = await api.polls
                .confirm(
                  guildId,
                  pollId,
                  confirmation.option.id,
                  confirmation.version,
                  input,
                )
                .catch((error: unknown) => {
                  refetchPermissionsOnBotError(client, guildId, error);
                  throw error;
                });
              setAnnouncement(result.announcement);
              await Promise.all([
                changed(),
                invalidateEvents(client, dashboardEventsSource, guildId, true),
              ]);
              return result.event;
            }}
          />
        </>
      )}
    </main>
  );
}
function VoteTable({ poll }: { poll: PollDetail }) {
  const ids = [...new Set(poll.votes.map((v) => v.user_id))].sort();
  const profiles = useQuery({
    queryKey: queryKeys.guild.members(poll.guild_id, ids),
    queryFn: async ({ signal }) => {
      const members: MemberProfile[] = [];
      // 20人ずつ逐次取得し、参加者が多くてもDiscord照会を一斉に発生させない。
      for (let i = 0; i < ids.length; i += 20) {
        signal.throwIfAborted();
        try {
          members.push(
            ...(await api.guilds.members(
              poll.guild_id,
              ids.slice(i, i + 20),
              signal,
            )),
          );
        } catch (error) {
          if (signal.aborted) throw error;
          // 失敗した組だけID表示にし、ほかの参加者名は取得する。
        }
      }
      return members;
    },
    enabled: ids.length > 0,
    staleTime: 60_000,
    retry: false,
  });
  const members = new Map(
    (profiles.data ?? []).map((member) => [member.user_id, member]),
  );
  const votes = new Map(
    poll.votes.map((vote) => [
      `${vote.user_id}/${vote.option_id}`,
      vote.answer,
    ]),
  );
  return (
    <section className="space-y-3">
      <h2 className="font-semibold">みんなの回答</h2>
      {ids.length === 0 ? (
        <p className="text-sm text-muted-foreground">まだ回答はありません。</p>
      ) : (
        <section
          className="overflow-x-auto rounded-lg border"
          // biome-ignore lint/a11y/noNoninteractiveTabindex: 横スクロールする表をキーボードで操作する
          tabIndex={0}
          aria-label="候補と参加者の回答表"
        >
          <table className="w-full text-sm">
            <caption className="sr-only">
              ○ 参加できる、△ 未定、× 参加できない、— 未回答
            </caption>
            <thead>
              <tr>
                <th className="p-3 text-left" scope="col">
                  参加者
                </th>
                {poll.options.map((o, i) => (
                  <th key={o.id} scope="col" className="whitespace-nowrap p-3">
                    候補 {i + 1}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {ids.map((id) => {
                const member = members.get(id);
                return (
                  <tr key={id} className="border-t">
                    <th scope="row" className="p-3 text-left font-normal">
                      <span className="flex items-center gap-2">
                        {member?.avatar_url && (
                          <Avatar url={member.avatar_url} />
                        )}
                        <span className="max-w-40 break-words">
                          {member?.display_name ?? id}
                          {id === poll.current_user_id && "（自分）"}
                        </span>
                      </span>
                    </th>
                    {poll.options.map((o) => {
                      const answer = votes.get(`${id}/${o.id}`);
                      return (
                        <td key={o.id} className="p-3 text-center">
                          {answer
                            ? { yes: "○", maybe: "△", no: "×" }[answer]
                            : "—"}
                        </td>
                      );
                    })}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </section>
      )}
    </section>
  );
}
function Avatar({ url }: { url: string }) {
  // biome-ignore lint/performance/noImgElement: Discord CDN のアバターを直接表示する
  return <img src={url} alt="" className="size-6 shrink-0 rounded-full" />;
}
function Announcement({ status }: { status: PollAnnouncement | undefined }) {
  if (!status) return null;
  return (
    <p role="status" className="rounded-lg bg-muted p-3 text-sm">
      {status === "sent"
        ? "Discordの通知チャンネルに案内を投稿しました。"
        : status === "not_configured"
          ? "保存しました。通知チャンネルが未設定のため、Discordへの案内は投稿していません。"
          : "保存しましたが、Discordへの案内投稿に失敗しました。このページのURLを共有してください。"}
    </p>
  );
}
function dateTime(value: string) {
  return `${value.slice(0, 10)} ${value.slice(11, 16)}`;
}
function optionLabel(option: PollOption) {
  return option.is_all_day
    ? `${option.start_at.slice(0, 10)} 〜 ${option.end_at.slice(0, 10)}（終日）`
    : `${dateTime(option.start_at)} 〜 ${dateTime(option.end_at)}`;
}
