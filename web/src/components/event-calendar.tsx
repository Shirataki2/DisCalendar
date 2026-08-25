"use client";

import Calendar, {
  type CalendarRef,
  type DateClickInfo,
  type DateSelectInfo,
  type DatesSetInfo,
  type EventChangeInfo,
  type EventClickInfo,
  type FormatterInput,
} from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/react/daygrid";
import interactionPlugin from "@fullcalendar/react/interaction";
import jaLocale from "@fullcalendar/react/locales/ja";
import classicThemePlugin from "@fullcalendar/react/themes/classic";
import timeGridPlugin from "@fullcalendar/react/timegrid";
import { addHours, format, startOfHour } from "date-fns";
import { PlusIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
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

/**
 * 日時の表記。FullCalendar の既定は Intl.DateTimeFormat 任せなので、日本語ロケールでは
 * ブラウザ (ICU) の実装差がそのまま出る。時刻は 24 時間制の "H:mm"、日付は数字だけに固定する
 */

/** "2:00" (分が 0 でも省略しない) */
function timeText({ hour, minute }: { hour: number; minute: number }): string {
  return `${hour}:${String(minute).padStart(2, "0")}`;
}

/**
 * 予定の時刻。既定は分が 0 のとき「時」だけを出す (omitZeroMinute) ので Chrome は "2時"、
 * Safari は "2:00" になる。終了時刻まで出す週 / 4日 / 日ビューでは範囲の書式
 * (Intl の formatRange。ja は "2時00分〜3時00分") が使われる
 */
const eventTimeFormat: FormatterInput = (info) =>
  info.end
    ? `${timeText(info.start)} - ${timeText(info.end)}`
    : timeText(info.start);

/** 時刻軸のラベル ("2:00")。既定は予定の時刻と同じ理由で "2時" と "2:00" に割れる */
const slotHeaderFormat: FormatterInput = (info) => timeText(info.date);

/** 月ビューの日付セル ("26")。ja の Intl は "26日" を返す (既定の omitTrailing では「日」が落ちない) */
const dayCellFormat: FormatterInput = (info) => String(info.date.day);

/** 曜日の略称。FullCalendar が渡す marker はタイムゾーンを持たない UTC 基準の Date */
const weekdayFormat = new Intl.DateTimeFormat("ja-JP", {
  weekday: "short",
  timeZone: "UTC",
});

/** 週 / 4日 / 日ビューの日付ヘッダ ("23(日)")。既定は ja だと "23日(日)" / "23日日曜日" */
const dayHeaderFormat: FormatterInput = (info) =>
  `${info.date.day}(${weekdayFormat.format(info.date.marker)})`;

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
  // FullCalendar は描画時の new Date() で「今日」を決めるため、サーバー (本番コンテナ・CI は UTC) と
  // ブラウザ (Asia/Tokyo) で日付がずれる時間帯は SSR の「今日」のセル (aria-current="date") が
  // hydration 後も残る (React は属性の差分を patch しない)。カレンダーはマウント後にだけ描画して
  // ブラウザの日付で決める (#48)。予定はもともとクライアントで取得しており SSR で出す内容は無い
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

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
      <div className="min-h-0 flex-1">
        {mounted && (
          <Calendar
            ref={calendarRef}
            plugins={[
              dayGridPlugin,
              timeGridPlugin,
              interactionPlugin,
              classicThemePlugin,
            ]}
            locale={jaLocale}
            eventTimeFormat={eventTimeFormat}
            slotHeaderFormat={slotHeaderFormat}
            dayCellFormat={dayCellFormat}
            initialView="dayGridMonth"
            headerToolbar={{
              start: "prev,next today",
              center: "title",
              end: "dayGridMonth,timeGridWeek,timeGridFourDay,timeGridDay",
            }}
            views={{
              // 日付ヘッダに日付を出すのは timeGrid 系だけ (月ビューは曜日だけでよい)。
              // 親の "timeGrid" に指定すると週 / 4日 / 日ビューすべてに効く
              timeGrid: { dayHeaderFormat },
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
            // タッチの長押し (既定 1000ms) を短くして、旧版の touchend 相当の操作感に寄せる (#14)。
            // 選択 (selectLongPressDelay) とドラッグ (eventLongPressDelay) の両方の既定になる
            longPressDelay={400}
            height="100%"
            datesSet={handleDatesSet}
            select={handleSelect}
            dateClick={handleDateClick}
            eventClick={handleEventClick}
            eventChange={handleEventChange}
          />
        )}
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
