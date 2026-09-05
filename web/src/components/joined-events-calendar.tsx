"use client";

import Calendar, {
  type CalendarRef,
  type DatesSetInfo,
  type EventClickInfo,
} from "@fullcalendar/react";
import {
  type RefObject,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  calendarBaseOptions,
  datesSetToRange,
  useCalendarBase,
} from "@/components/calendar-base";
import { EventPopover, type PopoverAnchor } from "@/components/event-popover";
import { useCalendarShortcuts } from "@/hooks/use-calendar-shortcuts";
import { describeApiError } from "@/lib/api";
import { sourceOf, toCalendarEvent } from "@/lib/calendar-events";
import { assignGuildColors } from "@/lib/guild-colors";
import { type EventRange, useJoinedEventsQuery } from "@/lib/query/events";
import { cn } from "@/lib/utils";

/** 横断カレンダーに載せるサーバー (Bot 参加済み)。並び順で色を割り当てる */
export interface JoinedGuildSummary {
  id: string;
  name: string;
  iconUrl: string | null;
}

interface Props {
  guilds: JoinedGuildSummary[];
}

interface PopoverState {
  eventId: number;
  anchor: PopoverAnchor;
}

/**
 * 凡例のうち、折り畳んだ 1 行に収まらずに隠れているチップの数。
 * サーバーが多い利用者やスマホ幅でも凡例がカレンダーの高さを食い潰さないよう、凡例は既定で 1 行に
 * 折り畳み、あふれた分は「他 N サーバーを表示」で開く。何件あふれるかは幅と名前の長さで変わるので、
 * 描画後に実際の位置で数える (行の下端より下から始まるチップ = 隠れている)
 */
function useOverflowCount(
  listRef: RefObject<HTMLUListElement | null>,
  collapsed: boolean,
): number {
  const [count, setCount] = useState(0);
  useLayoutEffect(() => {
    const list = listRef.current;
    if (!list || !collapsed) {
      setCount(0);
      return;
    }
    const measure = () => {
      const bottom = list.getBoundingClientRect().bottom;
      let hidden = 0;
      for (const child of list.children) {
        if (child.getBoundingClientRect().top >= bottom - 1) hidden += 1;
      }
      setCount(hidden);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(list);
    return () => observer.disconnect();
    // チップの増減 (サーバー一覧は RSC が渡すのでページ内では変わらない) も高さの変化として
    // ResizeObserver が拾うので、依存に入れるのは折り畳みの状態だけでよい
  }, [listRef, collapsed]);
  return count;
}

/**
 * 参加している全サーバーの予定をまとめて表示する閲覧専用のカレンダー (#98)。
 * 予定はサーバーごとの色で塗り、上の凡例で見分ける。凡例のチップを押すとそのサーバーの予定を
 * 隠せる (この画面を開いている間だけ。保存はしない)。作成・編集・ドラッグはできず、
 * 予定のポップオーバーからそのサーバーのカレンダーへ移動して行う
 */
export function JoinedEventsCalendar({ guilds }: Props) {
  const calendarRef = useRef<CalendarRef>(null);
  const { initialView, firstDay, scrollTime } = useCalendarBase();
  // キーボードショートカット (#160)。閲覧専用なので "n" (新規作成) は渡さない
  useCalendarShortcuts({ calendarRef });
  const [range, setRange] = useState<EventRange | null>(null);
  const [popover, setPopover] = useState<PopoverState | null>(null);
  const [hiddenGuildIds, setHiddenGuildIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  // 凡例の折り畳み。既定は 1 行で、あふれたときだけ開閉ボタンを出す
  const [legendExpanded, setLegendExpanded] = useState(false);
  const legendRef = useRef<HTMLUListElement>(null);
  const overflowCount = useOverflowCount(legendRef, !legendExpanded);

  // guildIds はクエリキーに入るので参照を固定する。色は一覧の並び順で決まる
  const guildIds = useMemo(() => guilds.map((guild) => guild.id), [guilds]);
  const colors = useMemo(() => assignGuildColors(guildIds), [guildIds]);
  const guildById = useMemo(
    () => new Map(guilds.map((guild) => [guild.id, guild])),
    [guilds],
  );

  const eventsQuery = useJoinedEventsQuery(guildIds, range);
  const events = useMemo(
    () =>
      (eventsQuery.data ?? [])
        .filter((event) => !hiddenGuildIds.has(event.guild_id))
        .map((event) =>
          toCalendarEvent(event, colors.get(event.guild_id) ?? event.color),
        ),
    [eventsQuery.data, hiddenGuildIds, colors],
  );
  // ポップオーバーに出す予定はキャッシュから最新を引く (予定 ID は全サーバーを通して一意)
  const popoverEvent = useMemo(
    () =>
      popover
        ? (eventsQuery.data?.find((event) => event.id === popover.eventId) ??
          null)
        : null,
    [eventsQuery.data, popover],
  );
  const popoverGuild = popoverEvent
    ? guildById.get(popoverEvent.guild_id)
    : undefined;

  const handleDatesSet = (info: DatesSetInfo) =>
    setRange(datesSetToRange(info));

  // クリックで概要ポップオーバー (単独カレンダーと同じ)。予定の要素は再描画で差し替わるので位置を覚えておく
  const handleEventClick = (info: EventClickInfo) => {
    const source = sourceOf(info.event);
    if (!source) return;
    const rect = info.el.getBoundingClientRect();
    setPopover({
      eventId: source.id,
      anchor: { getBoundingClientRect: () => rect },
    });
  };

  const toggleGuild = (guildId: string) => {
    setPopover(null);
    setHiddenGuildIds((previous) => {
      const next = new Set(previous);
      if (next.has(guildId)) {
        next.delete(guildId);
      } else {
        next.add(guildId);
      }
      return next;
    });
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        {/* 凡例。既定は 1 行に折り畳み (あふれた分は隠す)、開いたときも数行分でスクロールさせて
            カレンダーの高さを食い潰さないようにする */}
        <ul
          ref={legendRef}
          aria-label="サーバーの凡例"
          className={cn(
            "flex min-w-0 flex-wrap gap-1.5",
            legendExpanded
              ? "max-h-32 overflow-y-auto"
              : "max-h-7 overflow-hidden",
          )}
        >
          {guilds.map((guild) => {
            const shown = !hiddenGuildIds.has(guild.id);
            const color = colors.get(guild.id);
            return (
              <li key={guild.id}>
                <button
                  type="button"
                  aria-pressed={shown}
                  title={
                    shown
                      ? "このサーバーの予定を隠す"
                      : "このサーバーの予定を表示する"
                  }
                  onClick={() => toggleGuild(guild.id)}
                  className={cn(
                    "flex items-center gap-1.5 rounded-full border border-border px-2.5 py-1 text-xs transition-colors hover:bg-foreground/10",
                    !shown && "opacity-50",
                  )}
                >
                  <span
                    aria-hidden
                    className="size-3 shrink-0 rounded-full border-2"
                    style={
                      shown
                        ? { backgroundColor: color, borderColor: color }
                        : { borderColor: color }
                    }
                  />
                  {guild.iconUrl && (
                    // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
                    <img
                      src={guild.iconUrl}
                      alt=""
                      className="size-4 shrink-0 rounded-full"
                    />
                  )}
                  <span className="max-w-40 truncate">{guild.name}</span>
                </button>
              </li>
            );
          })}
        </ul>
        {(legendExpanded || overflowCount > 0) && (
          <button
            type="button"
            aria-expanded={legendExpanded}
            onClick={() => setLegendExpanded((value) => !value)}
            className="shrink-0 text-xs text-muted-foreground underline hover:text-foreground"
          >
            {legendExpanded
              ? "凡例を折りたたむ"
              : `他 ${overflowCount} サーバーを表示`}
          </button>
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
      </div>
      {/* calendar-shell は globals.css の微調整の起点 (単独カレンダーと同じ) */}
      <div className="calendar-shell min-h-0 flex-1">
        {initialView && (
          <Calendar
            ref={calendarRef}
            {...calendarBaseOptions}
            initialView={initialView}
            firstDay={firstDay}
            scrollTime={scrollTime}
            events={events}
            editable={false}
            selectable={false}
            datesSet={handleDatesSet}
            eventClick={handleEventClick}
          />
        )}
      </div>

      <EventPopover
        event={popoverEvent}
        anchor={popover?.anchor ?? null}
        canEdit={false}
        color={popoverEvent ? colors.get(popoverEvent.guild_id) : undefined}
        guild={
          popoverGuild && {
            name: popoverGuild.name,
            iconUrl: popoverGuild.iconUrl,
            href: `/dashboard/${popoverGuild.id}`,
          }
        }
        onClose={() => setPopover(null)}
      />
    </div>
  );
}
