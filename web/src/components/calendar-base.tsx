"use client";

import type {
  CalendarOptions,
  DatesSetInfo,
  DayCellInfo,
  DayHeaderInfo,
  FormatterInput,
} from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/react/daygrid";
import interactionPlugin from "@fullcalendar/react/interaction";
import listPlugin from "@fullcalendar/react/list";
import jaLocale from "@fullcalendar/react/locales/ja";
import classicThemePlugin from "@fullcalendar/react/themes/classic";
import timeGridPlugin from "@fullcalendar/react/timegrid";
import { addHours, format, startOfHour } from "date-fns";
import { useEffect, useState } from "react";
import {
  readCalendarSettings,
  readLastCalendarView,
  saveLastCalendarView,
  useCalendarSettings,
} from "@/hooks/use-calendar-settings";
import { toApiDateTime } from "@/lib/calendar-events";
import {
  type CalendarView,
  parseCalendarView,
  resolveInitialView,
} from "@/lib/calendar-settings";
import {
  dayColorClass,
  dowColorClass,
  holidayNameOf,
} from "@/lib/japanese-holidays";
import type { EventRange } from "@/lib/query/events";
import { cn } from "@/lib/utils";

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

/**
 * リストビューの日付見出し ("8/23")。既定は ja だと "2026年8月23日"。
 * 曜日は右側の副見出し (listDayAltFormat の既定 "日曜日") が出すので、ここには入れない。
 * 月は 0 始まり (JS の Date と同じ) なので +1 する
 */
const listDayFormat: FormatterInput = (info) =>
  `${info.date.month + 1}/${info.date.day}`;

/*
 * 週末と日本の祝日の配色 (#97)。日曜・祝日は赤、土曜は青 (色は globals.css の cal-day-*)。
 * 月ビューは日付の数字 (dayCellTopInner)、週 / 4日 / 日ビューとリストビューは日付見出しに付ける
 */

/**
 * 月ビューの日付セル上部の器。既定は右寄せだが、祝日名の付くセルだけ数字の位置が
 * 左へずれて見えるので、全セルを左上詰めに揃える
 */
const dayCellTopClass = "cal-day-top";

/**
 * 月ビューの日付の数字。セル全体ではなく数字だけを色付けする。前後の月のセルは
 * 既定の減光をそのまま生かす (色を付けると当月と見分けにくくなる) ため塗らない。
 * 祝日はセル幅からあふれた祝日名を切り詰められるよう、flex の器 (cal-holiday-top) にする
 */
const dayCellTopInnerClass = (info: DayCellInfo) =>
  cn(
    !info.isOther && dayColorClass(info),
    holidayNameOf(info.date) && "cal-holiday-top",
  );

/**
 * 日付ヘッダ。月ビューのヘッダは曜日だけで特定の日付を指さない
 * (info.date には先頭週の日付が入っている) ので、祝日は見ずに曜日だけで塗る
 */
const dayHeaderClass = (info: DayHeaderInfo) =>
  info.view.type === "dayGridMonth"
    ? dowColorClass(info.dow)
    : dayColorClass(info);

/** リストビューの日付見出し (日付と曜日の行全体) */
const listDayHeaderClass = (info: { date: Date; dow: number }) =>
  dayColorClass(info);

/** 月ビューの日付セルの上部。祝日は日付の数字に続けて祝日名を出す */
const dayCellTopContent = (info: DayCellInfo) => {
  const name = holidayNameOf(info.date);
  if (!name) return true; // true = 既定の表示 (日付の数字だけ)。undefined だと空になる
  return (
    <>
      {info.text}
      <span className="cal-holiday-name">{name}</span>
    </>
  );
};

/**
 * サーバーのカレンダー (EventCalendar) と横断カレンダー (JoinedEventsCalendar、#98) で
 * 同じにする FullCalendar の設定 (プラグイン・日時の表記・祝日・ビュー・ツールバー・操作感)。
 * 予定の中身や編集の可否など画面ごとに違うものは各コンポーネントが足す
 */
export const calendarBaseOptions = {
  plugins: [
    dayGridPlugin,
    timeGridPlugin,
    listPlugin,
    interactionPlugin,
    classicThemePlugin,
  ],
  locale: jaLocale,
  eventTimeFormat,
  slotHeaderFormat,
  dayCellFormat,
  dayCellTopClass,
  dayCellTopInnerClass,
  dayCellTopContent,
  dayHeaderClass,
  listDayHeaderClass,
  headerToolbar: {
    start: "prev,next today",
    center: "title",
    end: "dayGridMonth,timeGridWeek,timeGridFourDay,timeGridDay,listMonth",
  },
  views: {
    // 日付ヘッダに日付を出すのは timeGrid 系だけ (月ビューは曜日だけでよい)。
    // 親の "timeGrid" に指定すると週 / 4日 / 日ビューすべてに効く
    timeGrid: { dayHeaderFormat },
    timeGridFourDay: {
      type: "timeGrid",
      duration: { days: 4 },
    },
    list: { listDayFormat },
  },
  buttons: {
    timeGridFourDay: { text: "4日" },
    // ja ロケールの既定は "予定リスト" で、スマートフォンではボタンの並びが折り返す
    listMonth: { text: "リスト" },
  },
  nowIndicator: true,
  snapDuration: "00:15",
  slotDuration: "00:30",
  // タッチの長押し (既定 1000ms) を短くして、旧版の touchend 相当の操作感に寄せる (#14)。
  // 選択 (selectLongPressDelay) とドラッグ (eventLongPressDelay) の両方の既定になる
  longPressDelay: 400,
  height: "100%",
} satisfies CalendarOptions;

/**
 * マウント後に決めるカレンダーの初期状態。
 *
 * FullCalendar は描画時の new Date() で「今日」を決めるため、サーバー (本番コンテナ・CI は UTC) と
 * ブラウザ (Asia/Tokyo) で日付がずれる時間帯は SSR の「今日」のセル (aria-current="date") が
 * hydration 後も残る (React は属性の差分を patch しない)。カレンダーはマウント後にだけ描画して
 * ブラウザの日付で決める (#48)。予定はもともとクライアントで取得しており SSR で出す内容は無い。
 * 最初に表示するビュー (#96) もこの遅延を利用して、マウント時に localStorage の設定から決める
 * (`initialView` が null の間は描画しない = 従来の mounted 相当)。
 * 週の開始曜日 (#96) は設定ダイアログでの変更を開いたまま反映できるよう購読する
 */
export function useCalendarBase(): {
  initialView: CalendarView | null;
  firstDay: 0 | 1;
  /** 時間軸の初期スクロール位置 (1 時間前の正時) */
  scrollTime: string;
} {
  const [initialView, setInitialView] = useState<CalendarView | null>(null);
  useEffect(() => {
    setInitialView(
      resolveInitialView(readCalendarSettings(), readLastCalendarView()),
    );
  }, []);
  const { settings } = useCalendarSettings();
  const [scrollTime] = useState(() =>
    format(startOfHour(addHours(new Date(), -1)), "HH:mm"),
  );
  return { initialView, firstDay: settings.firstDay, scrollTime };
}

/**
 * datesSet の共通処理。「前回開いていたビュー」(#96) のために、いま見ているビューを記録し、
 * 予定の取得範囲 (JST 文字列) を返す
 */
export function datesSetToRange(info: DatesSetInfo): EventRange {
  const view = parseCalendarView(info.view.type);
  if (view) saveLastCalendarView(view);
  return {
    start: toApiDateTime(info.start),
    end: toApiDateTime(info.end),
  };
}
