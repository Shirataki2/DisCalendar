"use client";

import Calendar, {
  type CalendarRef,
  type DateSelectInfo,
  type DatesSetInfo,
  type EventChangeInfo,
  type EventClickInfo,
} from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/react/daygrid";
import interactionPlugin from "@fullcalendar/react/interaction";
import jaLocale from "@fullcalendar/react/locales/ja";
import classicThemePlugin from "@fullcalendar/react/themes/classic";
import timeGridPlugin from "@fullcalendar/react/timegrid";
import { addHours, format, startOfHour } from "date-fns";
import { PlusIcon } from "lucide-react";
import { useMemo, useRef, useState } from "react";
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
import { useLastValue } from "@/hooks/use-last-value";
import { describeApiError } from "@/lib/api";
import type { ApiEvent, ApiEventInput } from "@/lib/api/types";
import {
  sourceOf,
  toApiDateTime,
  toApiEventInput,
  toCalendarEvent,
} from "@/lib/calendar-events";
import {
  defaultEventFormValues,
  type EventFormValues,
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

import "@fullcalendar/react/skeleton.css";
import "@fullcalendar/react/themes/classic/theme.css";
import "@fullcalendar/react/themes/classic/palette.css";

interface Props {
  guildId: string;
  /** false なら閲覧のみ (restricted モードで管理権限がない) */
  canEdit: boolean;
  /** 予定の取得元。管理コンソール (#35) からは admin 用 API に差し替える */
  eventsSource?: EventsSource;
}

interface PopoverState {
  eventId: number;
  anchor: PopoverAnchor;
}

export function EventCalendar({
  guildId,
  canEdit,
  eventsSource = dashboardEventsSource,
}: Props) {
  const calendarRef = useRef<CalendarRef>(null);
  const [range, setRange] = useState<EventRange | null>(null);
  const [popover, setPopover] = useState<PopoverState | null>(null);
  const [dialog, setDialog] = useState<EventDialogState | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ApiEvent | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [scrollTime] = useState(() =>
    format(startOfHour(addHours(new Date(), -1)), "HH:mm"),
  );

  const eventsQuery = useEventsQuery(guildId, range, eventsSource);
  const createEvent = useCreateEvent(guildId, eventsSource);
  const updateEvent = useUpdateEvent(guildId, eventsSource);
  const deleteEvent = useDeleteEvent(guildId, eventsSource);

  const events = useMemo(
    () => (eventsQuery.data ?? []).map(toCalendarEvent),
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

  const handleDatesSet = (info: DatesSetInfo) => {
    setRange({
      start: toApiDateTime(info.start),
      end: toApiDateTime(info.end),
    });
  };

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

  const handleSelect = (info: DateSelectInfo) => {
    calendarRef.current?.getApi().unselect();
    openCreate(newEventFormValues(info.start, info.end, info.allDay));
  };

  const openEdit = (event: ApiEvent) => {
    setPopover(null);
    setDialog({ mode: "edit", event });
  };

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
              ? undefined
              : "このサーバーでは管理権限を持つユーザーのみ予定を編集できます"
          }
          className="rounded-full bg-amber-700 px-5 font-semibold text-white hover:bg-amber-600"
        >
          <PlusIcon />
          新規作成
        </Button>
        {eventsQuery.isFetching && (
          <span className="text-xs text-neutral-400">読み込み中…</span>
        )}
        {eventsQuery.isError && (
          <span className="flex items-center gap-2 rounded-md bg-red-900/40 px-3 py-1.5 text-sm text-red-200">
            予定を取得できませんでした: {describeApiError(eventsQuery.error)}
            <button
              type="button"
              onClick={() => eventsQuery.refetch()}
              className="underline hover:text-white"
            >
              再試行
            </button>
          </span>
        )}
        {actionError && (
          <span className="flex items-center gap-2 rounded-md bg-red-900/40 px-3 py-1.5 text-sm text-red-200">
            {actionError}
            <button
              type="button"
              onClick={() => setActionError(null)}
              className="underline hover:text-white"
            >
              閉じる
            </button>
          </span>
        )}
      </div>
      <div className="min-h-0 flex-1">
        <Calendar
          ref={calendarRef}
          plugins={[
            dayGridPlugin,
            timeGridPlugin,
            interactionPlugin,
            classicThemePlugin,
          ]}
          locale={jaLocale}
          initialView="dayGridMonth"
          headerToolbar={{
            start: "prev,next today",
            center: "title",
            end: "dayGridMonth,timeGridWeek,timeGridFourDay,timeGridDay",
          }}
          views={{
            timeGridFourDay: {
              type: "timeGrid",
              duration: { days: 4 },
            },
          }}
          buttons={{ timeGridFourDay: { text: "4日" } }}
          events={events}
          editable={canEdit}
          selectable={canEdit}
          selectMirror
          nowIndicator
          snapDuration="00:15"
          slotDuration="00:30"
          scrollTime={scrollTime}
          longPressDelay={400}
          height="100%"
          datesSet={handleDatesSet}
          select={handleSelect}
          eventClick={handleEventClick}
          eventChange={handleEventChange}
        />
      </div>

      <EventPopover
        event={popoverEvent}
        anchor={popover?.anchor ?? null}
        canEdit={canEdit}
        onEdit={openEdit}
        onDelete={setDeleteTarget}
        onClose={() => setPopover(null)}
      />
      <EventFormDialog
        state={dialog}
        onClose={() => setDialog(null)}
        onSubmit={submitDialog}
        onDelete={setDeleteTarget}
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
