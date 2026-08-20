"use client";

import Calendar, {
  type CalendarRef,
  type EventChangeInfo,
  type EventClickInfo,
  type EventInput,
} from "@fullcalendar/react";
import dayGridPlugin from "@fullcalendar/react/daygrid";
import interactionPlugin from "@fullcalendar/react/interaction";
import jaLocale from "@fullcalendar/react/locales/ja";
import classicThemePlugin from "@fullcalendar/react/themes/classic";
import timeGridPlugin from "@fullcalendar/react/timegrid";
import { addDays, addHours, format, startOfHour } from "date-fns";
import { useRef, useState } from "react";

import "@fullcalendar/react/skeleton.css";
import "@fullcalendar/react/themes/classic/theme.css";
import "@fullcalendar/react/themes/classic/palette.css";

// 現行アプリのイベントカラーパレット
const EVENT_COLORS = [
  "#2196F3",
  "#3F51B5",
  "#673AB7",
  "#00BCD4",
  "#4CAF50",
  "#FF9800",
  "#757575",
];

const DATETIME = "yyyy-MM-dd'T'HH:mm:ss";
const DATE = "yyyy-MM-dd";

function sampleEvents(): EventInput[] {
  const today = new Date();
  const at = (dayOffset: number, hour: number, minute = 0) => {
    const d = addDays(today, dayOffset);
    d.setHours(hour, minute, 0, 0);
    return format(d, DATETIME);
  };
  return [
    {
      id: "1",
      title: "定例ミーティング",
      start: at(0, 10),
      end: at(0, 11, 30),
      color: EVENT_COLORS[0],
    },
    {
      id: "2",
      title: "レイド練習",
      start: at(0, 21),
      end: at(0, 23),
      color: EVENT_COLORS[2],
    },
    {
      id: "3",
      title: "ボドゲ会",
      start: at(1, 13),
      end: at(1, 18),
      color: EVENT_COLORS[5],
    },
    {
      id: "4",
      title: "夏合宿",
      start: format(addDays(today, 3), DATE),
      end: format(addDays(today, 6), DATE),
      allDay: true,
      color: EVENT_COLORS[4],
    },
    {
      id: "5",
      title: "サーバー記念日",
      start: format(addDays(today, 1), DATE),
      allDay: true,
      color: EVENT_COLORS[3],
    },
  ];
}

interface SelectedEvent {
  id: string;
  title: string;
  range: string;
}

export function EventCalendar() {
  const calendarRef = useRef<CalendarRef>(null);
  const [selected, setSelected] = useState<SelectedEvent | null>(null);
  const [scrollTime] = useState(() =>
    format(startOfHour(addHours(new Date(), -1)), "HH:mm"),
  );

  const handleEventChange = (info: EventChangeInfo) => {
    // 本実装ではここで PUT /api/events/:guildId/:eventId を呼ぶ
    console.log("event changed:", info.event.toPlainObject());
  };

  const handleEventClick = (info: EventClickInfo) => {
    const { event } = info;
    const fmt = (d: Date | null) =>
      d ? format(d, event.allDay ? "M/d" : "M/d HH:mm") : "";
    setSelected({
      id: event.id,
      title: event.title,
      range: `${fmt(event.start)} - ${fmt(event.end)}`,
    });
  };

  const addEvent = () => {
    const api = calendarRef.current?.getApi();
    const start = startOfHour(addHours(new Date(), 1));
    api?.addEvent({
      id: crypto.randomUUID(),
      title: "新しい予定",
      start: format(start, DATETIME),
      end: format(addHours(start, 1), DATETIME),
      color: EVENT_COLORS[Math.floor(Math.random() * EVENT_COLORS.length)],
    });
  };

  const removeSelected = () => {
    if (!selected) return;
    if (!confirm("削除してもよろしいですか？")) return;
    // 本実装ではここで DELETE /api/events/:guildId/:eventId を呼ぶ
    calendarRef.current?.getApi().getEventById(selected.id)?.remove();
    setSelected(null);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onClick={addEvent}
          className="rounded-full bg-amber-700 px-5 py-2 text-sm font-semibold transition-colors hover:bg-amber-600"
        >
          ＋ 新規作成
        </button>
        {selected && (
          <div className="flex items-center gap-3 rounded-md bg-surface px-4 py-1.5 text-sm">
            <span className="font-semibold">{selected.title}</span>
            <span className="text-neutral-400">{selected.range}</span>
            <button
              type="button"
              onClick={removeSelected}
              className="text-red-400 hover:text-red-300"
            >
              削除
            </button>
            <button
              type="button"
              onClick={() => setSelected(null)}
              className="text-neutral-400 hover:text-neutral-200"
            >
              閉じる
            </button>
          </div>
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
          initialEvents={sampleEvents()}
          editable
          nowIndicator
          snapDuration="00:15"
          slotDuration="00:30"
          scrollTime={scrollTime}
          longPressDelay={400}
          height="100%"
          eventClick={handleEventClick}
          eventChange={handleEventChange}
        />
      </div>
    </div>
  );
}
