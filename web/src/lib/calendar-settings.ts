/**
 * カレンダーの表示設定 (#96)。テーマ (#58) と同じ「ブラウザに記憶する個人設定」で、
 * localStorage に端末ごとに保存する (DB には持たない)。
 * localStorage の読み書きと購読はクライアント専用の @/hooks/use-calendar-settings に置き、
 * ここには定数と純粋な関数だけを置く
 */

/** FullCalendar のビュー名 (ヘッダツールバーの並びと同じ) */
export const CALENDAR_VIEWS = [
  "dayGridMonth",
  "timeGridWeek",
  "timeGridFourDay",
  "timeGridDay",
  "listMonth",
] as const;

export type CalendarView = (typeof CALENDAR_VIEWS)[number];

/** 最初に表示するビューの設定値。"last" は「前回開いていたビュー」 */
export type CalendarInitialView = CalendarView | "last";

/** 週の開始曜日 (FullCalendar の firstDay)。0 = 日曜、1 = 月曜 */
export type CalendarFirstDay = 0 | 1;

export interface CalendarSettings {
  initialView: CalendarInitialView;
  firstDay: CalendarFirstDay;
}

/** 既定値は設定を追加する前の挙動 (月表示・日曜始まり) と同じにする */
export const DEFAULT_CALENDAR_SETTINGS: CalendarSettings = {
  initialView: "dayGridMonth",
  firstDay: 0,
};

/** 設定の保存先 (localStorage) のキー。値は CalendarSettings の JSON */
export const CALENDAR_SETTINGS_STORAGE_KEY = "discalendar-calendar-settings";

/** 前回開いていたビューの保存先 (localStorage) のキー。値はビュー名そのまま */
export const CALENDAR_LAST_VIEW_STORAGE_KEY = "discalendar-calendar-last-view";

function isCalendarView(value: unknown): value is CalendarView {
  return (CALENDAR_VIEWS as readonly unknown[]).includes(value);
}

/** ビュー名として妥当なら返す (前回開いていたビューの読み出しに使う) */
export function parseCalendarView(raw: string | null): CalendarView | null {
  return isCalendarView(raw) ? raw : null;
}

/**
 * localStorage の JSON を設定に起こす。手で書き換えられたり将来の値が入っていても
 * 落ちないよう、項目ごとに妥当な値だけ拾って残りは既定値にする
 */
export function parseCalendarSettings(raw: string | null): CalendarSettings {
  const settings = { ...DEFAULT_CALENDAR_SETTINGS };
  if (raw === null) return settings;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return settings;
  }
  if (typeof parsed !== "object" || parsed === null) return settings;
  const { initialView, firstDay } = parsed as Record<string, unknown>;
  if (initialView === "last" || isCalendarView(initialView)) {
    settings.initialView = initialView;
  }
  if (firstDay === 0 || firstDay === 1) {
    settings.firstDay = firstDay;
  }
  return settings;
}

/** 最初に表示するビューを決める。「前回開いていたビュー」は記録が無ければ既定の月にする */
export function resolveInitialView(
  settings: CalendarSettings,
  lastView: CalendarView | null,
): CalendarView {
  if (settings.initialView === "last") return lastView ?? "dayGridMonth";
  return settings.initialView;
}
