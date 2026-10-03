"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CheckCircle2Icon,
  CheckIcon,
  ChevronLeftIcon,
  ClockIcon,
  LockIcon,
  PencilIcon,
  PlusIcon,
  Trash2Icon,
} from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { EventDescription } from "@/components/event-description";
import {
  type EventDialogState,
  EventFormDialog,
} from "@/components/event-form-dialog";
import { useLanguage } from "@/components/language-provider";
import { PollFormDialog } from "@/components/poll-form-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { readCalendarSettings } from "@/hooks/use-calendar-settings";
import { api, describeApiError } from "@/lib/api";
import type {
  MemberProfile,
  PollAnnouncement,
  PollAnswer,
  PollDetail,
  PollOption,
  SchedulePoll,
} from "@/lib/api/types";
import { parseApiDateTime } from "@/lib/calendar-events";
import { newEventFormValues } from "@/lib/event-form";
import { formatDisplayDate, type Language } from "@/lib/i18n";
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
import { cn } from "@/lib/utils";

type PollStatus = SchedulePoll["status"];
/** ステータスごとの表示。進行中 = 緑、締切済み = 琥珀、確定済み = 青で一覧・詳細を揃える */
const statusStyles: Record<
  PollStatus,
  { label: string; badge: string; accent: string; surface: string; dot: string }
> = {
  open: {
    label: "進行中",
    badge:
      "bg-emerald-100 text-emerald-800 dark:bg-emerald-950 dark:text-emerald-300",
    accent: "border-l-emerald-500 dark:border-l-emerald-400",
    surface:
      "border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-900 dark:bg-emerald-950/40 dark:text-emerald-100",
    dot: "bg-emerald-500",
  },
  closed: {
    label: "締切済み",
    badge: "bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-300",
    accent: "border-l-amber-500 dark:border-l-amber-400",
    surface:
      "border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-100",
    dot: "bg-amber-500",
  },
  confirmed: {
    label: "確定済み",
    badge: "bg-sky-100 text-sky-800 dark:bg-sky-950 dark:text-sky-300",
    accent: "border-l-sky-500 dark:border-l-sky-400",
    surface:
      "border-sky-200 bg-sky-50 text-sky-950 dark:border-sky-900 dark:bg-sky-950/40 dark:text-sky-100",
    dot: "bg-sky-500",
  },
};
/** 回答ごとの色。押した回答ボタン・集計・回答表で共通にする */
const answers: {
  value: PollAnswer;
  mark: string;
  label: string;
  selected: string;
  chip: string;
  text: string;
}[] = [
  {
    value: "yes",
    mark: "○",
    label: "○ 参加できる",
    selected:
      "bg-emerald-600 text-white hover:bg-emerald-600/90 dark:bg-emerald-500 dark:text-emerald-950",
    chip: "bg-emerald-100 text-emerald-800 dark:bg-emerald-950 dark:text-emerald-300",
    text: "text-emerald-700 dark:text-emerald-400",
  },
  {
    value: "maybe",
    mark: "△",
    label: "△ 未定",
    selected:
      "bg-amber-500 text-white hover:bg-amber-500/90 dark:bg-amber-400 dark:text-amber-950",
    chip: "bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-300",
    text: "text-amber-700 dark:text-amber-400",
  },
  {
    value: "no",
    mark: "×",
    label: "× 参加できない",
    selected:
      "bg-rose-600 text-white hover:bg-rose-600/90 dark:bg-rose-500 dark:text-rose-950",
    chip: "bg-rose-100 text-rose-800 dark:bg-rose-950 dark:text-rose-300",
    text: "text-rose-700 dark:text-rose-400",
  },
];
/** 締切を過ぎた進行中の調整は、API の更新を待たずに締切済みとして表示する */
function effectiveStatus(poll: SchedulePoll): PollStatus {
  const expired =
    !!poll.deadline && Date.parse(`${poll.deadline}+09:00`) <= Date.now();
  return poll.status === "open" && expired ? "closed" : poll.status;
}
function StatusBadge({ status }: { status: PollStatus }) {
  const { t } = useLanguage();
  return (
    <Badge className={statusStyles[status].badge}>
      <span
        aria-hidden
        className={cn("size-1.5 rounded-full", statusStyles[status].dot)}
      />
      {t(statusStyles[status].label)}
    </Badge>
  );
}
export function PollList({ guildId }: { guildId: string }) {
  const { t, language } = useLanguage();
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
    <main
      lang={language}
      className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4 sm:p-6"
    >
      <Link
        href={`/dashboard/${guildId}`}
        className="inline-flex min-h-11 items-center gap-1 text-sm underline-offset-4 hover:underline"
      >
        <ChevronLeftIcon aria-hidden className="size-4" />
        {t("カレンダーに戻る")}
      </Link>
      <Card>
        <CardHeader>
          <CardTitle>
            <h1 className="text-2xl font-bold">{t("日程調整")}</h1>
          </CardTitle>
          <CardDescription>
            {t(
              "候補日への回答を集め、そのまま予定にできます。最新100件を表示します。",
            )}
          </CardDescription>
          <CardAction>
            <Button
              className="min-h-11"
              disabled={!canEdit}
              onClick={() => setCreating(true)}
            >
              <PlusIcon />
              {t("日程調整を作成")}
            </Button>
          </CardAction>
        </CardHeader>
      </Card>
      {query.isPending && (
        <Card>
          <CardContent>
            <p role="status">{t("読み込み中…")}</p>
          </CardContent>
        </Card>
      )}
      {query.isError && (
        <Card>
          <CardContent>
            <p role="alert" className="flex flex-wrap items-center gap-2">
              {describeApiError(query.error, language)}
              <Button variant="outline" onClick={() => query.refetch()}>
                {t("再試行")}
              </Button>
            </p>
          </CardContent>
        </Card>
      )}
      {query.data && (
        <div className="grid gap-4 lg:grid-cols-3">
          {(["open", "closed", "confirmed"] as const).map((status) => {
            const polls = query.data.filter(
              (p) => effectiveStatus(p) === status,
            );
            return (
              <Card
                key={status}
                size="sm"
                role="region"
                aria-labelledby={`polls-${status}`}
              >
                <CardHeader className="flex items-center gap-2">
                  <span
                    aria-hidden
                    className={cn(
                      "size-2.5 rounded-full",
                      statusStyles[status].dot,
                    )}
                  />
                  <h2 id={`polls-${status}`} className="font-semibold">
                    {t(statusStyles[status].label)}
                  </h2>
                  <Badge variant="secondary" className="ml-auto">
                    {t("{count}件", { count: polls.length })}
                  </Badge>
                </CardHeader>
                <CardContent>
                  {polls.length ? (
                    <ul className="space-y-2">
                      {polls.map((p) => (
                        <li key={p.id}>
                          <Link
                            className={cn(
                              "block rounded-lg border border-l-4 bg-background p-3 transition-colors hover:bg-muted focus-visible:outline-2 focus-visible:outline-ring",
                              statusStyles[status].accent,
                            )}
                            href={`/dashboard/${guildId}/polls/${p.id}`}
                          >
                            <span className="block break-words font-medium">
                              {p.title}
                            </span>
                            <span className="mt-1 flex items-center gap-1 text-xs text-muted-foreground">
                              <ClockIcon aria-hidden className="size-3.5" />
                              {t("締切: ")}
                              {p.deadline
                                ? dateTime(p.deadline, language)
                                : t("なし")}
                            </span>
                          </Link>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="rounded-lg border border-dashed p-3 text-center text-sm text-muted-foreground">
                      {t("日程調整はありません。")}
                    </p>
                  )}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
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
  const { t, language } = useLanguage();
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
  const status = poll ? effectiveStatus(poll) : "open";
  const open = !!poll && status === "open";
  const topYes = Math.max(0, ...(poll?.options.map((o) => o.yes) ?? []));
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
    <main
      lang={language}
      className="min-h-0 min-w-0 flex-1 space-y-4 overflow-y-auto p-4 sm:p-6"
    >
      <Link
        href={`/dashboard/${guildId}/polls`}
        className="inline-flex min-h-11 items-center gap-1 text-sm underline-offset-4 hover:underline"
      >
        <ChevronLeftIcon aria-hidden className="size-4" />
        {t("日程調整の一覧に戻る")}
      </Link>
      {query.isPending && (
        <Card>
          <CardContent>
            <p role="status">{t("読み込み中…")}</p>
          </CardContent>
        </Card>
      )}
      {query.isError && (
        <Card>
          <CardContent>
            <p role="alert" className="flex flex-wrap items-center gap-2">
              {describeApiError(query.error, language)}
              <Button variant="outline" onClick={() => query.refetch()}>
                {t("再試行")}
              </Button>
            </p>
          </CardContent>
        </Card>
      )}
      {poll && (
        <>
          <Card
            className={cn(
              "border-l-4 border-transparent",
              statusStyles[status].accent,
            )}
          >
            <CardHeader>
              <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                <StatusBadge status={status} />
                {t("日本時間")}
              </div>
              <h1 className="break-words text-2xl font-bold">{poll.title}</h1>
              <p className="flex items-center gap-1 text-sm text-muted-foreground">
                <ClockIcon aria-hidden className="size-4" />
                {t("締切: ")}
                {poll.deadline ? dateTime(poll.deadline, language) : t("なし")}
              </p>
            </CardHeader>
            {(poll.description ||
              announcement ||
              poll.status === "confirmed" ||
              !open) && (
              <CardContent className="space-y-3">
                {poll.description && (
                  <EventDescription>{poll.description}</EventDescription>
                )}
                <Announcement status={announcement} />
                {poll.status === "confirmed" && (
                  <p
                    className={cn(
                      "flex flex-wrap items-center gap-x-2 rounded-lg border p-3 text-sm",
                      statusStyles.confirmed.surface,
                    )}
                  >
                    <CheckCircle2Icon aria-hidden className="size-4" />
                    {t("日程が確定しました。")}
                    <Link
                      href={`/dashboard/${guildId}`}
                      className="font-medium underline underline-offset-4"
                    >
                      {t("カレンダーを見る")}
                    </Link>
                  </p>
                )}
                {!open && poll.status !== "confirmed" && (
                  <p
                    role="status"
                    className={cn(
                      "flex items-center gap-2 rounded-lg border p-3 text-sm",
                      statusStyles.closed.surface,
                    )}
                  >
                    <LockIcon aria-hidden className="size-4 shrink-0" />
                    {t(
                      "投票は締め切られています。集計から予定を確定できます。",
                    )}
                  </p>
                )}
              </CardContent>
            )}
            {canEdit && (
              <CardFooter className="flex-wrap gap-2 [&>button]:min-h-11">
                {open && (
                  <>
                    <Button
                      variant="outline"
                      disabled={action.isPending}
                      onClick={() => setEditing(true)}
                    >
                      <PencilIcon />
                      {t("候補を編集")}
                    </Button>
                    <Button
                      variant="outline"
                      disabled={action.isPending}
                      onClick={() => {
                        if (window.confirm(t("投票を締め切りますか？")))
                          action.mutate(() =>
                            api.polls.close(guildId, pollId, poll.version),
                          );
                      }}
                    >
                      <LockIcon />
                      {t("投票を締め切る")}
                    </Button>
                  </>
                )}
                <Button
                  variant="destructive"
                  className="sm:ml-auto"
                  disabled={action.isPending}
                  onClick={() => {
                    if (
                      window.confirm(
                        t(
                          "日程調整と回答を削除しますか？ 確定済みの予定は残ります。",
                        ),
                      )
                    )
                      action.mutate(async () => {
                        await api.polls.remove(guildId, pollId);
                        router.push(`/dashboard/${guildId}/polls`);
                      });
                  }}
                >
                  <Trash2Icon />
                  {t("日程調整を削除")}
                </Button>
              </CardFooter>
            )}
          </Card>
          {action.isError && (
            <p
              role="alert"
              className="rounded-lg bg-destructive/10 p-3 text-sm text-destructive"
            >
              {describeApiError(action.error, language)}
            </p>
          )}
          {canEdit && (config.isError || guild.isError) && (
            <p
              role="alert"
              className="flex flex-wrap items-center gap-2 rounded-lg bg-destructive/10 p-3 text-sm text-destructive"
            >
              {t("保存先・通知設定を取得できませんでした。")}
              <Button
                variant="outline"
                onClick={() => {
                  void config.refetch();
                  void guild.refetch();
                }}
              >
                {t("再取得")}
              </Button>
            </p>
          )}
          <Card role="region" aria-labelledby="poll-options-title">
            <CardHeader>
              <CardTitle>
                <h2 id="poll-options-title" className="font-semibold">
                  {t("候補と自分の回答")}
                </h2>
              </CardTitle>
              <CardDescription>
                {open
                  ? t(
                      "候補ごとに ○ / △ / × を選んでください。何度でも変更できます。",
                    )
                  : t("回答の受付は終了しました。")}
              </CardDescription>
            </CardHeader>
            <CardContent>
              <ol className="grid gap-3 md:grid-cols-2">
                {poll.options.map((option, index) => {
                  const mine = poll.votes.find(
                    (v) =>
                      v.option_id === option.id &&
                      v.user_id === poll.current_user_id,
                  )?.answer;
                  const leading = topYes > 0 && option.yes === topYes;
                  return (
                    <li
                      key={option.id}
                      className={cn(
                        "flex flex-col gap-3 rounded-lg border p-3",
                        leading && "border-emerald-300 dark:border-emerald-800",
                      )}
                    >
                      <div className="flex items-start gap-2">
                        <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-muted text-xs font-medium">
                          {index + 1}
                        </span>
                        <h3 className="min-w-0 flex-1 break-words font-medium">
                          <span className="sr-only">
                            {t("候補 {index}", { index: index + 1 })}:{" "}
                          </span>
                          {optionLabel(option, language)}
                        </h3>
                        {leading && (
                          <Badge className={answers[0].chip}>
                            {t("○が最多")}
                          </Badge>
                        )}
                      </div>
                      <p
                        className="flex flex-wrap gap-1.5 text-xs"
                        aria-live="polite"
                      >
                        {answers.map((answer) => (
                          <span
                            key={answer.value}
                            className={cn(
                              "rounded-full px-2 py-0.5 font-medium",
                              answer.chip,
                            )}
                          >
                            {answer.mark}{" "}
                            {t("{count}人", { count: option[answer.value] })}
                          </span>
                        ))}
                      </p>
                      <fieldset className="grid grid-cols-3 gap-1 rounded-lg bg-muted p-1">
                        <legend className="sr-only">
                          {t("候補 {index} への回答", { index: index + 1 })}
                        </legend>
                        {answers.map((answer) => (
                          <Button
                            key={answer.value}
                            type="button"
                            variant="ghost"
                            className={cn(
                              "h-auto min-h-11 whitespace-normal px-1 text-xs sm:text-sm",
                              mine === answer.value
                                ? answer.selected
                                : "bg-background hover:bg-background/70",
                            )}
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
                            {t(answer.label)}
                          </Button>
                        ))}
                      </fieldset>
                      {canEdit && poll.status !== "confirmed" && (
                        <Button
                          variant="secondary"
                          className="mt-auto min-h-11 self-end"
                          disabled={
                            action.isPending ||
                            !config.isSuccess ||
                            !guild.isSuccess
                          }
                          onClick={() => confirm(option, poll)}
                        >
                          <CheckIcon />
                          {t("この候補で確定")}
                        </Button>
                      )}
                    </li>
                  );
                })}
              </ol>
            </CardContent>
          </Card>
          <VoteTable poll={poll} />
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
                {t(
                  "選んだ候補で予定を作成します。色・通知・Discordイベント連携を設定できます。",
                )}
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
  const { t } = useLanguage();
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
    <Card role="region" aria-labelledby="poll-votes-title">
      <CardHeader>
        <CardTitle>
          <h2 id="poll-votes-title" className="font-semibold">
            {t("みんなの回答")}
          </h2>
        </CardTitle>
        <CardDescription>
          {t("{count}人が回答しています。", { count: ids.length })}
        </CardDescription>
      </CardHeader>
      <CardContent>
        {ids.length === 0 ? (
          <p className="rounded-lg border border-dashed p-3 text-center text-sm text-muted-foreground">
            {t("まだ回答はありません。")}
          </p>
        ) : (
          <section
            className="overflow-x-auto rounded-lg border"
            // biome-ignore lint/a11y/noNoninteractiveTabindex: 横スクロールする表をキーボードで操作する
            tabIndex={0}
            aria-label={t("候補と参加者の回答表")}
          >
            <table className="w-full text-sm">
              <caption className="sr-only">
                {t("○ 参加できる、△ 未定、× 参加できない、— 未回答")}
              </caption>
              <thead className="bg-muted/50">
                <tr>
                  <th className="p-3 text-left" scope="col">
                    {t("参加者")}
                  </th>
                  {poll.options.map((o, i) => (
                    <th
                      key={o.id}
                      scope="col"
                      className="whitespace-nowrap p-3"
                    >
                      {t("候補 {index}", { index: i + 1 })}
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
                            {id === poll.current_user_id && t("（自分）")}
                          </span>
                        </span>
                      </th>
                      {poll.options.map((o) => {
                        const answer = answers.find(
                          (a) => a.value === votes.get(`${id}/${o.id}`),
                        );
                        return (
                          <td
                            key={o.id}
                            className={cn(
                              "p-3 text-center font-semibold",
                              answer ? answer.text : "text-muted-foreground",
                            )}
                          >
                            {answer ? answer.mark : "—"}
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
      </CardContent>
    </Card>
  );
}
function Avatar({ url }: { url: string }) {
  // biome-ignore lint/performance/noImgElement: Discord CDN のアバターを直接表示する
  return <img src={url} alt="" className="size-6 shrink-0 rounded-full" />;
}
function Announcement({ status }: { status: PollAnnouncement | undefined }) {
  const { t } = useLanguage();
  if (!status) return null;
  return (
    <p
      role="status"
      className={cn(
        "rounded-lg border p-3 text-sm",
        status === "failed"
          ? "border-destructive/30 bg-destructive/10 text-destructive"
          : "bg-muted",
      )}
    >
      {status === "sent"
        ? t("Discordの通知チャンネルに案内を投稿しました。")
        : status === "not_configured"
          ? t(
              "保存しました。通知チャンネルが未設定のため、Discordへの案内は投稿していません。",
            )
          : t(
              "保存しましたが、Discordへの案内投稿に失敗しました。このページのURLを共有してください。",
            )}
    </p>
  );
}
function dateTime(value: string, language: Language, allDay = false) {
  if (language === "ja")
    return allDay
      ? value.slice(0, 10)
      : `${value.slice(0, 10)} ${value.slice(11, 16)}`;
  return formatDisplayDate(
    parseApiDateTime(value),
    language,
    allDay ? {} : { hour: "numeric", minute: "2-digit" },
  );
}
function optionLabel(option: PollOption, language: Language) {
  return option.is_all_day
    ? `${dateTime(option.start_at, language, true)} 〜 ${dateTime(option.end_at, language, true)}${language === "ja" ? "（終日）" : " (All day)"}`
    : `${dateTime(option.start_at, language)} 〜 ${dateTime(option.end_at, language)}`;
}
