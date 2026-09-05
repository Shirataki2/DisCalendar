"use client";

import Calendar, {
  type CalendarRef,
  type DateClickInfo,
  type DateSelectInfo,
  type DatesSetInfo,
  type EventChangeInfo,
  type EventClickInfo,
} from "@fullcalendar/react";
import { PlusIcon } from "lucide-react";
import { useMemo, useRef, useState } from "react";
import {
  calendarBaseOptions,
  datesSetToRange,
  useCalendarBase,
} from "@/components/calendar-base";
import {
  type EventDialogState,
  EventFormDialog,
} from "@/components/event-form-dialog";
import { EventPopover, type PopoverAnchor } from "@/components/event-popover";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { useCalendarShortcuts } from "@/hooks/use-calendar-shortcuts";
import { useLastValue } from "@/hooks/use-last-value";
import { describeApiError } from "@/lib/api";
import type { ApiEvent, ApiEventInput } from "@/lib/api/types";
import {
  sourceOf,
  toApiEventInput,
  toCalendarEvent,
} from "@/lib/calendar-events";
import {
  defaultEventFormValues,
  type EventFormValues,
  eventToFormValues,
  newEventFormValues,
} from "@/lib/event-form";
import {
  dashboardEventsSource,
  type EventRange,
  type EventsSource,
  useCreateEvent,
  useDeleteEvent,
  useEventsQuery,
  useUpdateEvent,
} from "@/lib/query/events";

interface Props {
  guildId: string;
  /** false なら閲覧のみ (restricted モードで管理権限がない) */
  canEdit: boolean;
  /** 予定の取得元。管理コンソール (#35) からは admin 用 API に差し替える */
  eventsSource?: EventsSource;
  /** ダイアログの「Discord のイベントとしても作成する」(#94)。未指定なら出さない */
  discordSync?: {
    botCreateEvents: boolean;
    canCreateEvents: boolean;
    /** 権限を取り直す (#122)。渡すと権限不足のときに「権限を再確認」ボタンを出す */
    onRefresh?: () => Promise<unknown>;
  };
}

interface PopoverState {
  eventId: number;
  anchor: PopoverAnchor;
}

export function EventCalendar({
  guildId,
  canEdit,
  eventsSource = dashboardEventsSource,
  discordSync,
}: Props) {
  const calendarRef = useRef<CalendarRef>(null);
  const [range, setRange] = useState<EventRange | null>(null);
  const [popover, setPopover] = useState<PopoverState | null>(null);
  const [dialog, setDialog] = useState<EventDialogState | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ApiEvent | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  // 初期ビュー (#48 / #96) と週の開始曜日は横断カレンダーと共通 (calendar-base.tsx)
  const { initialView, firstDay, scrollTime } = useCalendarBase();

  const eventsQuery = useEventsQuery(guildId, range, eventsSource);
  const createEvent = useCreateEvent(guildId, eventsSource);
  const updateEvent = useUpdateEvent(guildId, eventsSource);
  const deleteEvent = useDeleteEvent(guildId, eventsSource);

  const events = useMemo(
    () => (eventsQuery.data ?? []).map((event) => toCalendarEvent(event)),
    [eventsQuery.data],
  );
  // ポップオーバーに出す予定はキャッシュから最新を引く (ドラッグ後などに古い内容を出さない)
  const popoverEvent = useMemo(
    () =>
      popover
        ? (eventsQuery.data?.find((event) => event.id === popover.eventId) ??
          null)
        : null,
    [eventsQuery.data, popover],
  );
  // 確認ダイアログを閉じるアニメーションの間も名前を出しておく
  const deleteShown = useLastValue(deleteTarget);

  const handleDatesSet = (info: DatesSetInfo) =>
    setRange(datesSetToRange(info));

  // ドラッグ移動 / リサイズ。FullCalendar 側は既に動いているので、保存に失敗したら戻す
  const handleEventChange = (info: EventChangeInfo) => {
    const source = sourceOf(info.event);
    if (!source) return;
    setActionError(null);
    updateEvent.mutate(
      { id: source.id, input: toApiEventInput(info.event, source) },
      {
        onError: (error) => {
          info.revert();
          setActionError(describeApiError(error));
        },
      },
    );
  };

  // クリックで概要ポップオーバー (旧実装の右クリック / 長押し相当)。
  // 予定の要素は再描画で差し替わるので、クリック時点の位置を仮想要素として覚えておく
  const handleEventClick = (info: EventClickInfo) => {
    const source = sourceOf(info.event);
    if (!source) return;
    const rect = info.el.getBoundingClientRect();
    setPopover({
      eventId: source.id,
      anchor: { getBoundingClientRect: () => rect },
    });
  };

  const openCreate = (values: EventFormValues) => {
    setPopover(null);
    setDialog({ mode: "create", values });
  };

  // キーボードショートカット (#160)。"n" は「新規作成」ボタンと同じで、閲覧のみなら効かない
  useCalendarShortcuts({
    calendarRef,
    onCreate: canEdit ? () => openCreate(defaultEventFormValues()) : undefined,
  });

  const handleSelect = (info: DateSelectInfo) => {
    calendarRef.current?.getApi().unselect();
    openCreate(newEventFormValues(info.start, info.end, info.allDay));
  };

  // タッチのタップで日付から作成する (#14)。タッチでは select が長押し必須なので、タップは dateClick で拾う。
  // マウスのクリックは select が拾うので、ここで扱うと二重に開いてしまう (jsEvent の実体はタッチ由来なら
  // TouchEvent)。dateClick は selectable と無関係に発火するため、編集可否も自前で確認する
  const handleDateClick = (info: DateClickInfo) => {
    if (!canEdit) return;
    if (
      !(typeof TouchEvent !== "undefined" && info.jsEvent instanceof TouchEvent)
    )
      return;
    openCreate(newEventFormValues(info.date, null, info.allDay));
  };

  const openEdit = (event: ApiEvent) => {
    setPopover(null);
    setDialog({ mode: "edit", event });
  };

  // 複製は元の内容を初期値にした「作成」として扱う (#91)。保存は作成 API をそのまま使う
  const openDuplicate = (event: ApiEvent) =>
    openCreate(eventToFormValues(event));

  // ダイアログからの保存。失敗したら reject してダイアログ側でエラー表示する
  const submitDialog = (input: ApiEventInput) =>
    dialog?.mode === "edit"
      ? updateEvent.mutateAsync({ id: dialog.event.id, input })
      : createEvent.mutateAsync(input);

  const confirmDelete = async () => {
    if (!deleteTarget) return;
    const { id } = deleteTarget;
    setDeleteTarget(null);
    setPopover(null);
    setDialog(null);
    setActionError(null);
    try {
      await deleteEvent.mutateAsync(id);
    } catch (error) {
      setActionError(describeApiError(error));
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <Button
          type="button"
          size="lg"
          onClick={() => openCreate(defaultEventFormValues())}
          disabled={!canEdit}
          title={
            canEdit
              ? "新規作成 (n)"
              : "このサーバーでは管理権限を持つユーザーのみ予定を編集できます"
          }
          className="rounded-full bg-amber-700 px-5 font-semibold text-white hover:bg-amber-600"
        >
          <PlusIcon />
          新規作成
        </Button>
        {eventsQuery.isFetching && (
          <span className="text-xs text-muted-foreground">読み込み中…</span>
        )}
        {eventsQuery.isError && (
          <span className="flex items-center gap-2 rounded-md bg-destructive/10 px-3 py-1.5 text-sm text-destructive">
            予定を取得できませんでした: {describeApiError(eventsQuery.error)}
            <button
              type="button"
              onClick={() => eventsQuery.refetch()}
              className="underline hover:text-foreground"
            >
              再試行
            </button>
          </span>
        )}
        {actionError && (
          <span className="flex items-center gap-2 rounded-md bg-destructive/10 px-3 py-1.5 text-sm text-destructive">
            {actionError}
            <button
              type="button"
              onClick={() => setActionError(null)}
              className="underline hover:text-foreground"
            >
              閉じる
            </button>
          </span>
        )}
      </div>
      {/* calendar-shell は globals.css の微調整の起点 (FullCalendar のクラス名はハッシュで指せない) */}
      <div className="calendar-shell min-h-0 flex-1">
        {initialView && (
          <Calendar
            ref={calendarRef}
            {...calendarBaseOptions}
            initialView={initialView}
            firstDay={firstDay}
            scrollTime={scrollTime}
            events={events}
            editable={canEdit}
            selectable={canEdit}
            selectMirror
            datesSet={handleDatesSet}
            select={handleSelect}
            dateClick={handleDateClick}
            eventClick={handleEventClick}
            eventChange={handleEventChange}
          />
        )}
      </div>

      <EventPopover
        resolveAuthors={eventsSource === dashboardEventsSource}
        event={popoverEvent}
        anchor={popover?.anchor ?? null}
        canEdit={canEdit}
        onEdit={openEdit}
        onDuplicate={openDuplicate}
        onDelete={setDeleteTarget}
        onClose={() => setPopover(null)}
      />
      <EventFormDialog
        state={dialog}
        allowShare={canEdit && eventsSource === dashboardEventsSource}
        onClose={() => setDialog(null)}
        onSubmit={submitDialog}
        onDelete={setDeleteTarget}
        discordSync={discordSync}
      />
      <AlertDialog
        open={deleteTarget !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteTarget(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>予定を削除しますか？</AlertDialogTitle>
            <AlertDialogDescription>
              「{deleteShown?.name}」を削除します。この操作は取り消せません。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>キャンセル</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={confirmDelete}>
              削除
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
