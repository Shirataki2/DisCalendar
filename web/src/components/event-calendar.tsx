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
import { addDays, format } from "date-fns";
import { ChevronDownIcon, FileUpIcon, PlusIcon } from "lucide-react";
import {
  type CSSProperties,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  calendarBaseOptions,
  datesSetToRange,
  useCalendarBase,
} from "@/components/calendar-base";
import { CalendarLegendChip } from "@/components/calendar-legend-chip";
import {
  type EventDialogState,
  EventFormDialog,
} from "@/components/event-form-dialog";
import { EventPopover, type PopoverAnchor } from "@/components/event-popover";
import { IcsImportDialog } from "@/components/ics-import-dialog";
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  Popover,
  PopoverContent,
  PopoverDescription,
  PopoverTitle,
} from "@/components/ui/popover";
import {
  readCalendarSettings,
  useCalendarSettings,
} from "@/hooks/use-calendar-settings";
import { useCalendarShortcuts } from "@/hooks/use-calendar-shortcuts";
import { useLastValue } from "@/hooks/use-last-value";
import { describeApiError } from "@/lib/api";
import type {
  ApiEvent,
  ApiEventInput,
  ChangeScope,
  ExternalEvent,
  Notification,
} from "@/lib/api/types";
import {
  describeEventRange,
  parseApiDateTime,
  sourceOf,
  toApiEventInput,
  toCalendarEvent,
} from "@/lib/calendar-events";
import type { CalendarSettings } from "@/lib/calendar-settings";
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
import { useExternalEvents } from "@/lib/query/external-calendars";
import { cn } from "@/lib/utils";

interface Props {
  guildId: string;
  guildName: string;
  /** false なら閲覧のみ (restricted モードで管理権限も編集ロールもない) */
  canEdit: boolean;
  /** URL から指定された初期表示日と、表示後に開く予定。通常のサーバーカレンダーだけで使う */
  initialDate?: string;
  initialEventId?: number;
  /**
   * 新規作成の事前通知の初期値 (サーバー設定の「新しい予定の既定の事前通知」、#181)。
   * 未指定なら (管理コンソールなど) 旧フォームの既定 (1 日前と 1 時間前)
   */
  defaultNotifications?: readonly Notification[];
  /** 予定の取得元。管理コンソール (#35) からは admin 用 API に差し替える */
  eventsSource?: EventsSource;
  /** 練習用などの固定設定。指定時は個人設定を使わず、表示モードも保存しない。 */
  settingsOverride?: CalendarSettings;
  /** 案内は操作中のダイアログ内へ移し、フォーカスと重なりを干渉させない。 */
  guide?: {
    content: ReactNode;
    inlineContent: ReactNode;
    target?: "create" | number;
    date?: string;
  };
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
  guidance?: ReactNode;
  state: QuickAddState;
  onClose: () => void;
  onDetails: (values: EventFormValues) => void;
  onTitleChange: (title: string) => void;
  onSubmit: (input: ApiEventInput) => Promise<unknown>;
}

function QuickAddPopover({
  guidance,
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
        backdrop
        anchor={state.anchor}
        align="start"
        sideOffset={8}
        initialFocus={inputRef}
        className="w-80 max-w-[calc(100vw-1rem)]"
      >
        <PopoverTitle>予定をクイック追加</PopoverTitle>
        <PopoverDescription>{range}</PopoverDescription>
        {guidance}
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
  guildName,
  canEdit,
  initialDate,
  initialEventId,
  defaultNotifications,
  eventsSource = dashboardEventsSource,
  settingsOverride,
  guide,
  discordSync,
}: Props) {
  const { settings: savedSettings } = useCalendarSettings();
  const settings = settingsOverride ?? savedSettings;
  const currentSettings = () => settingsOverride ?? readCalendarSettings();
  const calendarRef = useRef<CalendarRef>(null);
  const eventToOpen = useRef(initialEventId);
  const quickAddId = useRef(0);
  const [range, setRange] = useState<EventRange | null>(null);
  const [popover, setPopover] = useState<PopoverState | null>(null);
  const [externalPopover, setExternalPopover] = useState<{
    id: string;
    anchor: PopoverAnchor;
  } | null>(null);
  const [hiddenExternal, setHiddenExternal] = useState<ReadonlySet<number>>(
    () => new Set(),
  );
  const [quickAdd, setQuickAdd] = useState<QuickAddState | null>(null);
  const [dialog, setDialog] = useState<EventDialogState | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ApiEvent | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [importDefaultColor, setImportDefaultColor] = useState<string | null>(
    null,
  );
  // 初期ビュー (#48 / #96) と週の開始曜日は横断カレンダーと共通 (calendar-base.tsx)
  const { initialView, firstDay, scrollTime } =
    useCalendarBase(settingsOverride);
  const guideDate = guide?.date;
  useEffect(() => {
    if (guideDate) calendarRef.current?.getApi().gotoDate(guideDate);
  }, [guideDate]);

  const [scopeRequest, setScopeRequest] = useState<{
    choose: (scope: ChangeScope) => void;
    cancel?: () => void;
  } | null>(null);
  const [deleteScope, setDeleteScope] = useState<ChangeScope>("this");
  const previewRecurrence = useCallback(
    (
      start: string,
      rule: import("@/lib/api/types").RecurrenceRule,
      signal: AbortSignal,
    ) =>
      eventsSource.client.preview
        ? eventsSource.client.preview(
            guildId,
            start,
            rule,
            signal,
            dialog?.mode === "edit" && dialog.scope === "future"
              ? dialog.event.id
              : undefined,
          )
        : Promise.resolve([]),
    [eventsSource, guildId, dialog],
  );
  const eventsQuery = useEventsQuery(guildId, range, eventsSource);
  const externalEnabled =
    eventsSource === dashboardEventsSource && !settingsOverride;
  const externalQuery = useExternalEvents(guildId, range, externalEnabled);
  const createEvent = useCreateEvent(guildId, eventsSource);
  const updateEvent = useUpdateEvent(guildId, eventsSource);
  const deleteEvent = useDeleteEvent(guildId, eventsSource);

  const events = useMemo(() => {
    const current = (eventsQuery.data ?? []).map((event) => ({
      ...toCalendarEvent(event),
      classNames:
        event.id === guide?.target
          ? [
              "ring-2",
              "ring-indigo-300",
              "ring-offset-2",
              "ring-offset-background",
            ]
          : [],
    }));
    const external = externalEnabled
      ? (externalQuery.data ?? []).flatMap(({ calendar, events }) =>
          hiddenExternal.has(calendar.id)
            ? []
            : events.map(
                (event): EventInput => ({
                  id: `external:${event.id}`,
                  title: event.name,
                  start: event.is_all_day
                    ? event.start_at.slice(0, 10)
                    : event.start_at,
                  end: event.is_all_day
                    ? format(
                        addDays(parseApiDateTime(event.end_at), 1),
                        "yyyy-MM-dd",
                      )
                    : event.end_at,
                  allDay: event.is_all_day,
                  color: calendar.color,
                  textColor: readableTextColor(calendar.color),
                  editable: false,
                  extendedProps: { externalSource: event },
                }),
              ),
        )
      : [];
    if (!quickAdd) return [...current, ...external];
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
    return [...current, ...external, preview];
  }, [
    eventsQuery.data,
    externalQuery.data,
    externalEnabled,
    hiddenExternal,
    quickAdd,
    guide?.target,
  ]);
  const selectedExternal =
    externalPopover &&
    externalQuery.data
      ?.flatMap((calendar) => calendar.events)
      .find((event) => event.id === externalPopover.id);
  const selectedExternalCalendar = externalQuery.data?.find(
    (item) => item.calendar.id === selectedExternal?.calendar_id,
  )?.calendar;
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
    setRange(datesSetToRange(info, settingsOverride === undefined));
    setQuickAdd(null);
  };

  // ドラッグ移動 / リサイズ。FullCalendar 側は既に動いているので、保存に失敗したら戻す
  const handleEventChange = (info: EventChangeInfo) => {
    const source = sourceOf(info.event);
    if (!source) return;
    setActionError(null);
    const input = toApiEventInput(info.event, source);
    const save = (scope: ChangeScope) =>
      updateEvent.mutate(
        {
          id: source.id,
          input: {
            ...input,
            scope,
            expected_series_version: source.recurrence?.version,
          },
        },
        {
          onError: (error) => {
            info.revert();
            setActionError(describeApiError(error));
          },
        },
      );
    if (source.recurrence)
      setScopeRequest({ choose: save, cancel: () => info.revert() });
    else save("this");
  };

  // クリックで概要ポップオーバー (旧実装の右クリック / 長押し相当)。
  // 予定の要素は再描画で差し替わるので、クリック時点の位置を仮想要素として覚えておく
  const handleEventClick = (info: EventClickInfo) => {
    const external = info.event.extendedProps.externalSource as
      | ExternalEvent
      | undefined;
    if (external) {
      const rect = info.el.getBoundingClientRect();
      setPopover(null);
      setExternalPopover({
        id: external.id,
        anchor: { getBoundingClientRect: () => rect },
      });
      return;
    }
    const source = sourceOf(info.event);
    if (!source) return;
    const rect = info.el.getBoundingClientRect();
    setQuickAdd(null);
    setExternalPopover(null);
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
        currentSettings(),
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
        currentSettings(),
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
        currentSettings(),
      ),
      info.jsEvent,
      info.dayEl,
    );
  };

  const openEdit = (event: ApiEvent) => {
    setPopover(null);
    if (event.recurrence)
      setScopeRequest({
        choose: (scope) => setDialog({ mode: "edit", event, scope }),
      });
    else setDialog({ mode: "edit", event });
  };

  const openDelete = (event: ApiEvent) => {
    setPopover(null);
    setDeleteScope("this");
    setDeleteTarget(event);
  };

  // 複製は元の内容を初期値にした「作成」として扱う (#91)。保存は作成 API をそのまま使う
  const openDuplicate = (event: ApiEvent) => {
    setPopover(null);
    setDialog({ mode: "duplicate", values: eventToFormValues(event) });
  };

  // ダイアログからの保存。失敗したら reject してダイアログ側でエラー表示する
  const submitDialog = async (input: ApiEventInput) => {
    const saved =
      dialog?.mode === "edit"
        ? await updateEvent.mutateAsync({ id: dialog.event.id, input })
        : await createEvent.mutateAsync(input);
    // 添付だけが失敗しても、次の保存で同じ予定を更新する。
    setDialog((current) =>
      current === dialog ? { mode: "edit", event: saved } : current,
    );
    return saved;
  };

  const confirmDelete = async () => {
    if (!deleteTarget) return;
    const { id } = deleteTarget;
    setDeleteTarget(null);
    setPopover(null);
    setDialog(null);
    setActionError(null);
    try {
      await deleteEvent.mutateAsync({
        id,
        scope: deleteScope,
        expected_series_version: deleteTarget.recurrence?.version,
      });
    } catch (error) {
      setActionError(describeApiError(error));
    }
  };

  const overlayOpen = !!(
    dialog ||
    popoverEvent ||
    selectedExternal ||
    quickAdd ||
    deleteTarget ||
    scopeRequest ||
    importDefaultColor
  );
  return (
    <div
      className={cn(
        "flex min-h-0 min-w-0 flex-1 flex-col gap-5",
        guide && "lg:flex-row",
      )}
    >
      <div className="flex min-h-0 min-w-0 flex-1 flex-col gap-3">
        <div className="flex flex-wrap items-center gap-3">
          {eventsSource === dashboardEventsSource ? (
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button
                    type="button"
                    size="lg"
                    disabled={!canEdit}
                    title={
                      canEdit
                        ? "予定を新規作成、またはICSファイルから取り込む"
                        : "このサーバーでは管理権限または指定ロールを持つメンバーが予定を編集できます"
                    }
                    className={cn(
                      "rounded-full bg-amber-700 px-5 font-semibold text-white hover:bg-amber-600",
                      guide?.target === "create" &&
                        "ring-2 ring-indigo-300 ring-offset-4 ring-offset-background",
                    )}
                  />
                }
              >
                <PlusIcon />
                新規作成
                <ChevronDownIcon className="ml-1" />
              </DropdownMenuTrigger>
              <DropdownMenuContent className="w-64">
                <DropdownMenuItem
                  className="min-h-11"
                  onClick={openCreateDefault}
                >
                  <PlusIcon />
                  予定を作成
                  <DropdownMenuShortcut>n</DropdownMenuShortcut>
                </DropdownMenuItem>
                <DropdownMenuItem
                  className="min-h-11"
                  onClick={() =>
                    setImportDefaultColor(currentSettings().defaultColor)
                  }
                >
                  <FileUpIcon />
                  ICSファイルから取り込む
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : (
            <Button
              type="button"
              size="lg"
              onClick={openCreateDefault}
              disabled={!canEdit}
              title={canEdit ? "新規作成 (n)" : "予定を編集できません"}
              className="rounded-full bg-amber-700 px-5 font-semibold text-white hover:bg-amber-600"
            >
              <PlusIcon />
              新規作成
            </Button>
          )}
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
            <span
              role="alert"
              className="flex items-center gap-2 rounded-md bg-destructive/10 px-3 py-1.5 text-sm text-destructive"
            >
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
        {externalEnabled && (externalQuery.data?.length ?? 0) > 0 && (
          <ul
            aria-label="外部カレンダーの凡例"
            className="flex max-h-24 flex-wrap gap-1.5 overflow-y-auto"
          >
            {externalQuery.data?.map(({ calendar }) => {
              const shown = !hiddenExternal.has(calendar.id);
              return (
                <li key={calendar.id}>
                  <CalendarLegendChip
                    name={calendar.name}
                    color={calendar.color}
                    shown={shown}
                    error={calendar.last_error ? "取得できません" : null}
                    onClick={() => {
                      setExternalPopover(null);
                      setHiddenExternal((previous) => {
                        const next = new Set(previous);
                        if (shown) next.add(calendar.id);
                        else next.delete(calendar.id);
                        return next;
                      });
                    }}
                  />
                </li>
              );
            })}
          </ul>
        )}
        {externalEnabled && externalQuery.isError && (
          <p role="alert" className="text-sm text-destructive">
            外部カレンダーを取得できませんでした。
            <button
              type="button"
              className="underline"
              onClick={() => void externalQuery.refetch()}
            >
              再試行
            </button>
          </p>
        )}
        {/* calendar-shell は globals.css の微調整の起点 (FullCalendar のクラス名はハッシュで指せない) */}
        <div
          className="calendar-shell min-h-0 flex-1"
          style={
            {
              "--fc-classic-highlight": `color-mix(in srgb, ${settings.defaultColor} 20%, transparent)`,
            } as CSSProperties
          }
        >
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
              eventColor={settings.defaultColor}
              datesSet={handleDatesSet}
              selectMinDistance={5}
              select={handleSelect}
              dateClick={handleDateClick}
              eventClick={handleEventClick}
              eventDidMount={(info) => {
                const source = sourceOf(info.event);
                if (!source || source.id !== eventToOpen.current) return;
                eventToOpen.current = undefined;
                setPopover({ eventId: source.id, anchor: info.el });
              }}
              eventChange={handleEventChange}
            />
          )}
        </div>

        <EventPopover
          guidance={
            popoverEvent && !dialog && !deleteTarget
              ? guide?.inlineContent
              : null
          }
          allowAttachments={eventsSource === dashboardEventsSource}
          resolveAuthors={eventsSource === dashboardEventsSource}
          event={popoverEvent}
          anchor={popover?.anchor ?? null}
          canEdit={canEdit}
          onEdit={openEdit}
          onDuplicate={openDuplicate}
          onDelete={openDelete}
          onClose={() => setPopover(null)}
        />
        <Popover
          open={!!selectedExternal}
          onOpenChange={(open) => {
            if (!open) setExternalPopover(null);
          }}
        >
          {selectedExternal && (
            <PopoverContent
              anchor={externalPopover?.anchor}
              side="right"
              align="start"
              className="w-80 max-w-[calc(100vw-1rem)]"
            >
              <PopoverTitle>{selectedExternal.name}</PopoverTitle>
              <PopoverDescription>
                {selectedExternalCalendar?.name} · 外部カレンダーの予定
              </PopoverDescription>
              <p className="text-sm">{describeEventRange(selectedExternal)}</p>
              {selectedExternal.description && (
                <p className="max-h-40 overflow-y-auto whitespace-pre-wrap break-words text-sm text-muted-foreground">
                  {selectedExternal.description}
                </p>
              )}
            </PopoverContent>
          )}
        </Popover>
        {quickAdd && (
          <QuickAddPopover
            guidance={guide?.inlineContent}
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
          previewRecurrence={
            eventsSource.client.preview ? previewRecurrence : undefined
          }
          guildName={guildName}
          guidance={dialog && !deleteTarget ? guide?.inlineContent : null}
          mentionGuildId={
            eventsSource === dashboardEventsSource ? guildId : undefined
          }
          state={dialog}
          allowShare={canEdit && eventsSource === dashboardEventsSource}
          onClose={() => setDialog(null)}
          onSubmit={submitDialog}
          onDelete={openDelete}
          discordSync={discordSync}
        />
        {eventsSource === dashboardEventsSource && importDefaultColor && (
          <IcsImportDialog
            guildId={guildId}
            open
            onOpenChange={(open) => {
              if (!open) setImportDefaultColor(null);
            }}
            defaultColor={importDefaultColor}
          />
        )}
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
            {deleteTarget?.recurrence && (
              <fieldset className="space-y-2">
                <legend>削除する予定</legend>
                {(["this", "future"] as const).map((scope) => (
                  <label
                    key={scope}
                    className="flex min-h-11 items-center gap-2"
                  >
                    <input
                      type="radio"
                      name="delete-scope"
                      checked={deleteScope === scope}
                      onChange={() => setDeleteScope(scope)}
                    />
                    {scope === "this" ? "この回のみ" : "この回以降"}
                  </label>
                ))}
                {deleteScope === "future" && (
                  <p className="text-sm text-destructive">
                    元の開催日がこの回以降の予定を、個別編集済みの回と添付ファイルも含めて削除します。
                  </p>
                )}
              </fieldset>
            )}
            {deleteTarget && guide?.inlineContent}
            <AlertDialogFooter>
              <AlertDialogCancel>キャンセル</AlertDialogCancel>
              <AlertDialogAction variant="destructive" onClick={confirmDelete}>
                削除
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
        <AlertDialog
          open={scopeRequest !== null}
          onOpenChange={(open) => {
            if (!open) {
              scopeRequest?.cancel?.();
              setScopeRequest(null);
            }
          }}
        >
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>変更する予定</AlertDialogTitle>
              <AlertDialogDescription>
                変更を適用する範囲を選んでください。
              </AlertDialogDescription>
            </AlertDialogHeader>
            <div className="grid gap-3">
              {(["this", "future"] as const).map((scope) => (
                <Button
                  key={scope}
                  type="button"
                  variant="outline"
                  className="min-h-11"
                  onClick={() => {
                    const action = scopeRequest;
                    setScopeRequest(null);
                    action?.choose(scope);
                  }}
                >
                  {scope === "this" ? "この回のみ" : "この回以降"}
                </Button>
              ))}
            </div>
            <AlertDialogFooter>
              <AlertDialogCancel>キャンセル</AlertDialogCancel>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>
      {guide && (
        <aside
          aria-label="操作ガイド"
          className="order-first shrink-0 lg:order-last lg:w-80"
        >
          {!overlayOpen && guide.content}
        </aside>
      )}
    </div>
  );
}
