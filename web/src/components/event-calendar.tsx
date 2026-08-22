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
import { useMemo, useRef, useState } from "react";
import { describeApiError } from "@/lib/api";
import type { ApiEvent } from "@/lib/api/types";
import {
  describeNotification,
  parseApiDateTime,
  sourceOf,
  toApiDateTime,
  toApiEventInput,
  toApiRange,
  toCalendarEvent,
} from "@/lib/calendar-events";
import {
  type EventRange,
  useCreateEvent,
  useDeleteEvent,
  useEventsQuery,
  useUpdateEvent,
} from "@/lib/query/events";

import "@fullcalendar/react/skeleton.css";
import "@fullcalendar/react/themes/classic/theme.css";
import "@fullcalendar/react/themes/classic/palette.css";

/** 旧フォームの既定色 */
const DEFAULT_COLOR = "#F44336";
const NAME_MAX_CHARS = 32;

interface Props {
  guildId: string;
  /** false なら閲覧のみ (restricted モードで管理権限がない) */
  canEdit: boolean;
}

export function EventCalendar({ guildId, canEdit }: Props) {
  const calendarRef = useRef<CalendarRef>(null);
  const [range, setRange] = useState<EventRange | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [scrollTime] = useState(() =>
    format(startOfHour(addHours(new Date(), -1)), "HH:mm"),
  );

  const eventsQuery = useEventsQuery(guildId, range);
  const createEvent = useCreateEvent(guildId);
  const updateEvent = useUpdateEvent(guildId);
  const deleteEvent = useDeleteEvent(guildId);

  const events = useMemo(
    () => (eventsQuery.data ?? []).map(toCalendarEvent),
    [eventsQuery.data],
  );
  const selected = useMemo(
    () => eventsQuery.data?.find((event) => event.id === selectedId) ?? null,
    [eventsQuery.data, selectedId],
  );

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

  const handleEventClick = (info: EventClickInfo) => {
    setSelectedId(sourceOf(info.event)?.id ?? null);
  };

  // 新規作成ダイアログ (タイトル・通知・色・説明) は別途実装予定。
  // それまではタイトルだけ入力して作成し、日時はカレンダー上のドラッグで調整する
  const quickCreate = (start: Date, end: Date | null, allDay: boolean) => {
    const name = window.prompt("予定のタイトルを入力してください")?.trim();
    if (!name) return;
    if (name.length > NAME_MAX_CHARS) {
      setActionError(`タイトルは${NAME_MAX_CHARS}文字以内で入力してください`);
      return;
    }
    setActionError(null);
    createEvent.mutate(
      {
        name,
        description: null,
        notifications: [],
        color: DEFAULT_COLOR,
        is_all_day: allDay,
        ...toApiRange(start, end, allDay),
      },
      { onError: (error) => setActionError(describeApiError(error)) },
    );
  };

  const addEvent = () => {
    const start = startOfHour(addHours(new Date(), 1));
    quickCreate(start, addHours(start, 1), false);
  };

  const handleSelect = (info: DateSelectInfo) => {
    calendarRef.current?.getApi().unselect();
    quickCreate(info.start, info.end, info.allDay);
  };

  const removeSelected = () => {
    if (!selected) return;
    if (!confirm(`「${selected.name}」を削除してもよろしいですか？`)) return;
    setActionError(null);
    deleteEvent.mutate(selected.id, {
      onError: (error) => setActionError(describeApiError(error)),
    });
    setSelectedId(null);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={addEvent}
          disabled={!canEdit || createEvent.isPending}
          title={
            canEdit
              ? undefined
              : "このサーバーでは管理権限を持つユーザーのみ予定を編集できます"
          }
          className="rounded-full bg-amber-700 px-5 py-2 text-sm font-semibold transition-colors hover:bg-amber-600 disabled:cursor-not-allowed disabled:opacity-50"
        >
          ＋ 新規作成
        </button>
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
        {selected && (
          <SelectedEventPanel
            event={selected}
            canEdit={canEdit}
            onRemove={removeSelected}
            onClose={() => setSelectedId(null)}
          />
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
    </div>
  );
}

interface SelectedEventPanelProps {
  event: ApiEvent;
  canEdit: boolean;
  onRemove: () => void;
  onClose: () => void;
}

/** クリックした予定の概要 (旧実装の右クリックポップオーバー相当) */
function SelectedEventPanel({
  event,
  canEdit,
  onRemove,
  onClose,
}: SelectedEventPanelProps) {
  const notifications = event.notifications.length
    ? event.notifications.map(describeNotification).join(" ")
    : "-";
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-md bg-surface px-4 py-1.5 text-sm">
      <span
        className="inline-block h-3 w-3 rounded-full"
        style={{ backgroundColor: event.color }}
      />
      <span className="font-semibold">{event.name}</span>
      <span className="text-neutral-400">{describeRange(event)}</span>
      <span className="text-neutral-400" title="通知">
        🔔 {notifications}
      </span>
      {event.description && (
        <span className="max-w-xs truncate text-neutral-300">
          {event.description}
        </span>
      )}
      {canEdit && (
        <button
          type="button"
          onClick={onRemove}
          className="text-red-400 hover:text-red-300"
        >
          削除
        </button>
      )}
      <button
        type="button"
        onClick={onClose}
        className="text-neutral-400 hover:text-neutral-200"
      >
        閉じる
      </button>
    </div>
  );
}

/** 旧実装 (SimpleEdit.vue) と同じ表示形式 */
function describeRange(event: ApiEvent): string {
  const start = parseApiDateTime(event.start_at);
  const end = parseApiDateTime(event.end_at);
  const sameDay = format(start, "yyyy-MM-dd") === format(end, "yyyy-MM-dd");
  if (event.is_all_day) {
    return sameDay
      ? format(start, "yyyy/MM/dd")
      : `${format(start, "yyyy/MM/dd")} - ${format(end, "yyyy/MM/dd")}`;
  }
  return sameDay
    ? `${format(start, "yyyy/MM/dd HH:mm")} - ${format(end, "HH:mm")}`
    : `${format(start, "yyyy/MM/dd HH:mm")} - ${format(end, "yyyy/MM/dd HH:mm")}`;
}
