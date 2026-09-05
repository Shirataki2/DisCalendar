"use client";

import type { CalendarRef } from "@fullcalendar/react";
import { type RefObject, useEffect, useEffectEvent } from "react";
import { useOpenKeyboardShortcuts } from "@/components/keyboard-shortcuts-dialog";
import {
  isShortcutTargetBlocked,
  resolveCalendarShortcut,
} from "@/lib/calendar-shortcuts";

interface Options {
  calendarRef: RefObject<CalendarRef | null>;
  /** "n" で呼ぶ。渡さなければ (閲覧のみ・横断カレンダー) "n" は何もしない */
  onCreate?: () => void;
}

/**
 * 開いているダイアログ・ドロワー (Sheet)・確認ダイアログ。モーダルはフォーカスを閉じ込めるので
 * 普段は keydown の target で弾けるが、開く途中などフォーカスが外にある瞬間の保険として DOM でも見る
 */
const OPEN_OVERLAY_SELECTOR =
  ':is([data-slot="dialog-content"], [data-slot="sheet-content"], [data-slot="alert-dialog-content"])[data-open]';

/**
 * カレンダーのキーボードショートカット (#160)。サーバーのカレンダー (EventCalendar) と
 * 横断カレンダー (JoinedEventsCalendar) の両方から使う。キーの対応は lib/calendar-shortcuts.ts。
 * 期間の移動とビューの切替は FullCalendar の API を呼ぶだけで、ビューや予定の取得の仕組みは変えない。
 * Esc は Base UI のダイアログ / ポップオーバーが既定で閉じるので、ここでは扱わない
 */
export function useCalendarShortcuts({ calendarRef, onCreate }: Options): void {
  const openHelp = useOpenKeyboardShortcuts();

  // 最新の onCreate / openHelp を使いつつ、リスナーの付け外しは 1 回で済ませる
  const handleKeyDown = useEffectEvent((event: KeyboardEvent) => {
    if (event.defaultPrevented) return;
    const action = resolveCalendarShortcut(event);
    if (!action) return;
    if (
      isShortcutTargetBlocked(event.target) ||
      document.querySelector(OPEN_OVERLAY_SELECTOR)
    ) {
      return;
    }
    const api = calendarRef.current?.getApi();
    if (!api) return;
    switch (action.type) {
      case "today":
        api.today();
        break;
      case "prev":
        api.prev();
        break;
      case "next":
        api.next();
        break;
      case "view":
        api.changeView(action.view);
        break;
      case "create":
        if (!onCreate) return;
        onCreate();
        break;
      case "help":
        if (!openHelp) return;
        openHelp();
        break;
    }
    event.preventDefault();
  });

  useEffect(() => {
    const listener = (event: KeyboardEvent) => handleKeyDown(event);
    document.addEventListener("keydown", listener);
    return () => document.removeEventListener("keydown", listener);
  }, []);
}
