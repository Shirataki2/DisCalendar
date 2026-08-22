import type { EventApi, EventInput } from "@fullcalendar/react";
import {
  addDays,
  addHours,
  format,
  max,
  parseISO,
  startOfDay,
  subDays,
} from "date-fns";
import type { ApiEvent, ApiEventInput } from "@/lib/api/types";
import { readableTextColor } from "@/lib/color";

// API と FullCalendar の間の予定の変換。
// API の日時はタイムゾーンなしの JST 文字列で、ブラウザのローカル時刻をそのまま JST とみなす
// (旧実装の moment ベースの扱いと同じ)。

const API_DATETIME = "yyyy-MM-dd'T'HH:mm:ss";
const API_DATE = "yyyy-MM-dd";

export function toApiDateTime(date: Date): string {
  return format(date, API_DATETIME);
}

/** タイムゾーンなしの文字列なので、parseISO はローカル時刻として解釈する */
export function parseApiDateTime(value: string): Date {
  return parseISO(value);
}

export interface ApiDateRange {
  start_at: string;
  end_at: string;
}

/**
 * FullCalendar 上の期間 → API の start_at / end_at。
 * 終日予定は FullCalendar では end が「翌日 0:00 (含まない)」、DB では「終了日 (含む)」なので 1 日ずらす。
 * end が null (単日の終日予定 / 終了未指定) の場合も扱う
 */
export function toApiRange(
  start: Date,
  end: Date | null,
  allDay: boolean,
): ApiDateRange {
  if (allDay) {
    const first = startOfDay(start);
    const last = end ? subDays(startOfDay(end), 1) : first;
    return {
      start_at: toApiDateTime(first),
      end_at: toApiDateTime(max([first, last])),
    };
  }
  // 時間指定の予定で end がないのは「終日 → 時間指定」へドラッグした直後など。
  // FullCalendar の既定表示 (1 時間) と同じ長さで保存する
  return {
    start_at: toApiDateTime(start),
    end_at: toApiDateTime(end ?? addHours(start, 1)),
  };
}

/** API の予定 → FullCalendar の EventInput。元の予定は extendedProps.source に保持する */
export function toCalendarEvent(event: ApiEvent): EventInput {
  const base: EventInput = {
    id: String(event.id),
    title: event.name,
    color: event.color,
    textColor: readableTextColor(event.color),
    extendedProps: { source: event },
  };
  if (event.is_all_day) {
    const first = startOfDay(parseApiDateTime(event.start_at));
    const last = startOfDay(parseApiDateTime(event.end_at));
    return {
      ...base,
      allDay: true,
      start: format(first, API_DATE),
      end: format(addDays(max([first, last]), 1), API_DATE),
    };
  }
  return {
    ...base,
    allDay: false,
    start: event.start_at,
    end: event.end_at,
  };
}

/** FullCalendar の EventApi から元の API の予定を取り出す (toCalendarEvent で入れたもの) */
export function sourceOf(event: EventApi): ApiEvent | null {
  const source: unknown = event.extendedProps.source;
  return isApiEvent(source) ? source : null;
}

function isApiEvent(value: unknown): value is ApiEvent {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as ApiEvent).id === "number" &&
    typeof (value as ApiEvent).start_at === "string"
  );
}

/**
 * ドラッグ / リサイズ後の FullCalendar の予定から API の更新リクエストを作る。
 * 日時と終日フラグ以外は元の予定を引き継ぐ (更新 API は全フィールド置き換えのため)
 */
export function toApiEventInput(
  event: EventApi,
  source: ApiEvent,
): ApiEventInput {
  const start = event.start ?? parseApiDateTime(source.start_at);
  return {
    name: source.name,
    description: source.description,
    notifications: source.notifications,
    color: source.color,
    is_all_day: event.allDay,
    ...toApiRange(start, event.end, event.allDay),
  };
}

/** 通知設定の表示用文字列 ("30分前" など) */
export function describeNotification({
  num,
  unit,
}: ApiEvent["notifications"][number]): string {
  const label = {
    minutes: "分前",
    hours: "時間前",
    days: "日前",
    weeks: "週間前",
  }[unit];
  return `${num}${label}`;
}

/** 予定の期間の表示用文字列。旧実装 (SimpleEdit.vue) と同じ形式 */
export function describeEventRange(event: ApiEvent): string {
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
