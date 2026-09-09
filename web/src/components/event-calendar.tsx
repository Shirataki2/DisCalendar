"use client";

import Calendar, {
  type CalendarRef,
  type DateClickInfo,
  type DateSelectInfo,
  type DatesSetInfo,
  type EventChangeInfo,
  type EventClickInfo,
  type EventInput,
} from "@fullcalendar/react";
import { addDays } from "date-fns";
import { PlusIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
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
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverTitle,
} from "@/components/ui/popover";
import { readCalendarSettings } from "@/hooks/use-calendar-settings";
import { useCalendarShortcuts } from "@/hooks/use-calendar-shortcuts";
import { useLastValue } from "@/hooks/use-last-value";
import { describeApiError } from "@/lib/api";
import type { ApiEvent, ApiEventInput, Notification } from "@/lib/api/types";
import {
  describeEventRange,
  sourceOf,
  toApiEventInput,
  toCalendarEvent,
} from "@/lib/calendar-events";
import { readableTextColor } from "@/lib/color";
import {
  defaultEventFormValues,
  type EventFormValues,
  eventFormSchema,
  eventFormToApiInput,
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
  /** false なら閲覧のみ (restricted モードで管理権限も編集ロールもない) */
  canEdit: boolean;
  /**
   * 新規作成の事前通知の初期値 (サーバー設定の「新しい予定の既定の事前通知」、#181)。
   * 未指定なら (管理コンソールなど) 旧フォームの既定 (1 日前と 1 時間前)
   */
  defaultNotifications?: readonly Notification[];
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

interface QuickAddState {
  id: number;
  values: EventFormValues;
  title: string;
  anchor: PopoverAnchor;
}

interface QuickAddProps {
  state: QuickAddState;
  onClose: () => void;
  onDetails: (values: EventFormValues) => void;
  onTitleChange: (title: string) => void;
  onSubmit: (input: ApiEventInput) => Promise<unknown>;
}

function QuickAddPopover({
  state,
  onClose,
  onDetails,
  onTitleChange,
  onSubmit,
}: QuickAddProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const values = { ...state.values, name: state.title };
  const range = describeEventRange(
    eventFormToApiInput({ ...state.values, name: "-" }),
  );
  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    const parsed = eventFormSchema.safeParse(values);
    if (!parsed.success) {
      setError(
        parsed.error.issues.find((issue) => issue.path[0] === "name")
          ?.message ?? "入力内容を確認してください",
      );
      return;
    }
    setError(null);
    setIsSubmitting(true);
    try {
      await onSubmit(eventFormToApiInput(parsed.data));
      onClose();
    } catch (submitError) {
      setError(describeApiError(submitError));
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <Popover
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <PopoverContent
        anchor={state.anchor}
        align="start"
        sideOffset={8}
        initialFocus={inputRef}
        className="w-80 max-w-[calc(100vw-1rem)]"
      >
        <PopoverTitle>予定をクイック追加</PopoverTitle>
        <PopoverDescription>{range}</PopoverDescription>
        <form onSubmit={submit} noValidate className="flex flex-col gap-2">
          <label htmlFor="quick-add-title" className="sr-only">
            タイトル
          </label>
          <Input
            ref={inputRef}
            id="quick-add-title"
            value={state.title}
            placeholder="タイトルを入力して Enter"
            disabled={isSubmitting}
            aria-invalid={error ? true : undefined}
            onChange={(event) => {
              onTitleChange(event.target.value);
              setError(null);
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" && event.nativeEvent.isComposing)
                event.preventDefault();
            }}
          />
          {error && (
            <p role="alert" className="text-xs text-destructive">
              {error}
            </p>
          )}
          <div className="flex justify-end gap-2">
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={isSubmitting}
              onClick={() => onDetails(values)}
            >
              詳細を入力
            </Button>
            <Button type="submit" size="sm" disabled={isSubmitting}>
              {isSubmitting ? "作成中…" : "作成"}
            </Button>
          </div>
        </form>
      </PopoverContent>
    </Popover>
  );
}

export function EventCalendar({
  guildId,
  canEdit,
  defaultNotifications,
  eventsSource = dashboardEventsSource,
  discordSync,
}: Props) {
  const calendarRef = useRef<CalendarRef>(null);
  const quickAddId = useRef(0);
  const [range, setRange] = useState<EventRange | null>(null);
  const [popover, setPopover] = useState<PopoverState | null>(null);
  const [quickAdd, setQuickAdd] = useState<QuickAddState | null>(null);
  const [dialog, setDialog] = useState<EventDialogState | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ApiEvent | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  // 初期ビュー (#48 / #96) と週の開始曜日は横断カレンダーと共通 (calendar-base.tsx)
  const { initialView, firstDay, scrollTime } = useCalendarBase();
  const [initialDate, setInitialDate] = useState<string>();
  useEffect(() => {
    const date = new URLSearchParams(window.location.search).get("date");
    if (
      date &&
      /^\d{4}-\d{2}-\d{2}$/.test(date) &&
      !Number.isNaN(Date.parse(date))
    )
      setInitialDate(date);
  }, []);

  const eventsQuery = useEventsQuery(guildId, range, eventsSource);
  const createEvent = useCreateEvent(guildId, eventsSource);
  const updateEvent = useUpdateEvent(guildId, eventsSource);
  const deleteEvent = useDeleteEvent(guildId, eventsSource);

  const events = useMemo(() => {
    const current = (eventsQuery.data ?? []).map((event) =>
      toCalendarEvent(event),
    );
    if (!quickAdd) return current;
    const input = eventFormToApiInput({
      ...quickAdd.values,
      name: quickAdd.title,
    });
    const preview: EventInput = {
      id: "quick-add-preview",
      title: quickAdd.title,
      allDay: input.is_all_day,
      start: input.is_all_day ? quickAdd.values.startDate : input.start_at,
      end: input.is_all_day
        ? addDays(quickAdd.values.endDate, 1)
        : input.end_at,
      color: input.color,
      textColor: readableTextColor(input.color),
      editable: false,
      classNames: ["pointer-events-none"],
    };
    return [...current, preview];
  }, [eventsQuery.data, quickAdd]);
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
    setRange(datesSetToRange(info));
    setQuickAdd(null);
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
    setQuickAdd(null);
    setPopover({
      eventId: source.id,
      anchor: { getBoundingClientRect: () => rect },
    });
  };

  const openCreate = (values: EventFormValues) => {
    setPopover(null);
    setQuickAdd(null);
    setDialog({ mode: "create", values });
  };

  const openQuickAdd = (
    values: EventFormValues,
    event: MouseEvent | null,
    fallback?: HTMLElement,
  ) => {
    const rect = event
      ? new DOMRect(event.clientX, event.clientY)
      : (fallback?.getBoundingClientRect() ?? new DOMRect());
    setPopover(null);
    setQuickAdd({
      id: ++quickAddId.current,
      values,
      title: "",
      anchor: { getBoundingClientRect: () => rect },
    });
  };

  // 「新規作成」ボタンとショートカットからの初期値。操作時点の個人設定を使う。
  const openCreateDefault = () =>
    openCreate(
      defaultEventFormValues(
        new Date(),
        defaultNotifications,
        readCalendarSettings(),
      ),
    );

  // キーボードショートカット (#160)。"n" は「新規作成」ボタンと同じで、閲覧のみなら効かない
  useCalendarShortcuts({
    calendarRef,
    onCreate: canEdit ? openCreateDefault : undefined,
  });

  const handleSelect = (info: DateSelectInfo) => {
    calendarRef.current?.getApi().unselect();
    if (!canEdit) return;
    openQuickAdd(
      newEventFormValues(
        info.start,
        info.end,
        info.allDay,
        defaultNotifications,
        readCalendarSettings(),
      ),
      info.jsEvent,
    );
  };

  // クリックは終了未指定、ドラッグは select の範囲を使う。
  // selectMinDistance で単なるクリックによる select の二重発火を避ける。
  const handleDateClick = (info: DateClickInfo) => {
    if (!canEdit) return;
    openQuickAdd(
      newEventFormValues(
        info.date,
        null,
        info.allDay,
        defaultNotifications,
        readCalendarSettings(),
      ),
      info.jsEvent,
      info.dayEl,
    );
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
          onClick={openCreateDefault}
          disabled={!canEdit}
          title={
            canEdit
              ? "新規作成 (n)"
              : "このサーバーでは管理権限または指定ロールを持つメンバーが予定を編集できます"
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
            initialDate={initialDate}
            firstDay={firstDay}
            scrollTime={scrollTime}
            events={events}
            editable={canEdit}
            selectable={canEdit}
            selectMirror
            datesSet={handleDatesSet}
            selectMinDistance={5}
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
      {quickAdd && (
        <QuickAddPopover
          key={quickAdd.id}
          state={quickAdd}
          onClose={() => setQuickAdd(null)}
          onDetails={openCreate}
          onTitleChange={(title) =>
            setQuickAdd((state) => (state ? { ...state, title } : state))
          }
          onSubmit={(input) => createEvent.mutateAsync(input)}
        />
      )}
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
