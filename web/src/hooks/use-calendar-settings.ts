"use client";

import { useCallback, useMemo, useSyncExternalStore } from "react";
import {
  CALENDAR_LAST_VIEW_STORAGE_KEY,
  CALENDAR_SETTINGS_STORAGE_KEY,
  type CalendarSettings,
  type CalendarView,
  parseCalendarSettings,
  parseCalendarView,
} from "@/lib/calendar-settings";

/**
 * カレンダーの表示設定 (#96) の localStorage への読み書き。
 * next-themes に任せているテーマと違い、ここは自前で購読を組む。
 * storage イベントは他のタブの変更でしか発火しないので、同じタブ内の変更
 * (設定ダイアログ → カレンダー) は自前のリスナーで伝える
 */

const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  window.addEventListener("storage", listener);
  return () => {
    listeners.delete(listener);
    window.removeEventListener("storage", listener);
  };
}

// localStorage はプライベートブラウジングなどで例外になり得るので、読めなければ無い扱い、
// 書ければ儲けものとする (保存できない環境では設定がタブ限りになるだけ)
function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // 保存できなくても表示への反映 (emit) は行う
  }
}

function settingsSnapshot(): string | null {
  return read(CALENDAR_SETTINGS_STORAGE_KEY);
}

function serverSnapshot(): null {
  return null;
}

/** レンダー外 (effect など) から設定を読む。サーバーでは呼ばないこと */
export function readCalendarSettings(): CalendarSettings {
  return parseCalendarSettings(read(CALENDAR_SETTINGS_STORAGE_KEY));
}

/** 前回開いていたビューの記録を読む。無ければ (壊れていれば) null */
export function readLastCalendarView(): CalendarView | null {
  return parseCalendarView(read(CALENDAR_LAST_VIEW_STORAGE_KEY));
}

/**
 * 前回開いていたビューを記録する。設定が「前回開いていたビュー」以外のときも常に記録して、
 * あとから設定を変えたときに直近の操作が効くようにする。表示は変わらないので emit しない
 */
export function saveLastCalendarView(view: CalendarView): void {
  if (read(CALENDAR_LAST_VIEW_STORAGE_KEY) === view) return;
  write(CALENDAR_LAST_VIEW_STORAGE_KEY, view);
}

/**
 * カレンダーの表示設定を購読する。サーバーと hydration 直後は既定値になるので、
 * 描画に使う側は「マウント後にだけ描く」(event-calendar.tsx) などで SSR とのずれを避けること
 */
export function useCalendarSettings(): {
  settings: CalendarSettings;
  updateSettings: (patch: Partial<CalendarSettings>) => void;
} {
  const raw = useSyncExternalStore(subscribe, settingsSnapshot, serverSnapshot);
  const settings = useMemo(() => parseCalendarSettings(raw), [raw]);

  const updateSettings = useCallback((patch: Partial<CalendarSettings>) => {
    write(
      CALENDAR_SETTINGS_STORAGE_KEY,
      JSON.stringify({ ...readCalendarSettings(), ...patch }),
    );
    emit();
  }, []);

  return { settings, updateSettings };
}
