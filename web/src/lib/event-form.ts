import {
  addHours,
  addMinutes,
  format,
  isBefore,
  max,
  set,
  startOfDay,
  startOfHour,
  subDays,
} from "date-fns";
import { z } from "zod";
import type {
  ApiEvent,
  ApiEventInput,
  Notification,
  NotificationUnit,
} from "@/lib/api/types";
import { parseApiDateTime, toApiDateTime } from "@/lib/calendar-events";

// 予定の作成・編集フォーム (旧 NewEvent.vue) のスキーマと API との相互変換。
// 上限値は api/src/models/events.rs の validate() と揃えている

export const NAME_MAX_CHARS = 32;
export const DESCRIPTION_MAX_CHARS = 1000;
export const NOTIFICATIONS_MAX = 10;
export const NOTIFICATION_NUM_MIN = 1;
export const NOTIFICATION_NUM_MAX = 100;
/** 旧フォームの既定色 */
export const DEFAULT_COLOR = "#F44336";

/** 旧フォームの v-color-picker の swatches (4 列 × 5 行) */
export const COLOR_SWATCHES = [
  "#F44336",
  "#E91E63",
  "#9C27B0",
  "#673AB7",
  "#3F51B5",
  "#2196F3",
  "#03A9F4",
  "#00BCD4",
  "#009688",
  "#4CAF50",
  "#8BC34A",
  "#CDDC39",
  "#FFEB3B",
  "#FFC107",
  "#FF9800",
  "#FF5722",
  "#9E9E9E",
  "#212121",
  "#FF0000",
  "#0000FF",
] as const;

export const NOTIFICATION_UNITS: { value: NotificationUnit; label: string }[] =
  [
    { value: "weeks", label: "週間前" },
    { value: "days", label: "日前" },
    { value: "hours", label: "時間前" },
    { value: "minutes", label: "分前" },
  ];

/** 旧フォームの既定の通知 (1 日前と 1 時間前) */
const DEFAULT_NOTIFICATIONS: Notification[] = [
  { num: 1, unit: "days" },
  { num: 1, unit: "hours" },
];

const TIME_PATTERN = /^([01]\d|2[0-3]):[0-5]\d$/;
const HEX_COLOR_PATTERN = /^#[0-9A-Fa-f]{6}$/;
const NOTIFICATION_NUM_MESSAGE = `${NOTIFICATION_NUM_MIN}〜${NOTIFICATION_NUM_MAX}の範囲で入力してください`;

const notificationSchema = z.object({
  num: z
    .number({ error: "数値を入力してください" })
    .int("整数で入力してください")
    .min(NOTIFICATION_NUM_MIN, NOTIFICATION_NUM_MESSAGE)
    .max(NOTIFICATION_NUM_MAX, NOTIFICATION_NUM_MESSAGE),
  unit: z.enum([
    "minutes",
    "hours",
    "days",
    "weeks",
  ] as const satisfies readonly NotificationUnit[]),
});

export const eventFormSchema = z
  .object({
    name: z
      .string()
      .trim()
      .min(1, "タイトルを入力してください")
      .max(
        NAME_MAX_CHARS,
        `タイトルは${NAME_MAX_CHARS}文字以内で入力してください`,
      ),
    isAllDay: z.boolean(),
    startDate: z.date({ error: "開始日を選択してください" }),
    /** "HH:mm"。終日のときは使わない */
    startTime: z.string(),
    endDate: z.date({ error: "終了日を選択してください" }),
    /** "HH:mm"。終日のときは使わない */
    endTime: z.string(),
    color: z
      .string()
      .regex(HEX_COLOR_PATTERN, "色は #RRGGBB 形式で指定してください"),
    notifications: z
      .array(notificationSchema)
      .max(NOTIFICATIONS_MAX, `通知は${NOTIFICATIONS_MAX}件まで設定できます`),
    description: z
      .string()
      .max(
        DESCRIPTION_MAX_CHARS,
        `説明は${DESCRIPTION_MAX_CHARS}文字以内で入力してください`,
      ),
    /** Discord のスケジュールイベントとしても作成・同期する (#94) */
    discordEvent: z.boolean(),
  })
  .superRefine((values, ctx) => {
    if (!values.isAllDay) {
      let valid = true;
      if (!TIME_PATTERN.test(values.startTime)) {
        valid = false;
        ctx.addIssue({
          code: "custom",
          path: ["startTime"],
          message: "開始時刻を入力してください",
        });
      }
      if (!TIME_PATTERN.test(values.endTime)) {
        valid = false;
        ctx.addIssue({
          code: "custom",
          path: ["endTime"],
          message: "終了時刻を入力してください",
        });
      }
      if (!valid) return;
    }
    const { start, end } = toDateRange(values);
    if (isBefore(end, start)) {
      ctx.addIssue({
        code: "custom",
        path: [values.isAllDay ? "endDate" : "endTime"],
        message: "終了日時を開始日時より前にすることはできません",
      });
    }
  });

export type EventFormValues = z.infer<typeof eventFormSchema>;

type FormRange = Pick<
  EventFormValues,
  "isAllDay" | "startDate" | "startTime" | "endDate" | "endTime"
>;

function combine(date: Date, time: string): Date {
  const [hours, minutes] = time.split(":").map(Number);
  return set(startOfDay(date), { hours, minutes });
}

/**
 * フォームの開始日時。時刻が未入力・不正なら null。
 * Discord 連携 (#94) の「開始が過去なら連携できない」の判定に使う (api 側の検証と同じ条件)
 */
export function formStartAt(
  values: Pick<EventFormValues, "isAllDay" | "startDate" | "startTime">,
): Date | null {
  if (values.isAllDay) return startOfDay(values.startDate);
  if (!TIME_PATTERN.test(values.startTime)) return null;
  return combine(values.startDate, values.startTime);
}

/**
 * フォームの値 → 実際の開始/終了日時。
 * 終日予定は両端とも 0:00 で、終了日は「含む」(DB の表現と同じ)
 */
export function toDateRange(values: FormRange): { start: Date; end: Date } {
  if (values.isAllDay) {
    return {
      start: startOfDay(values.startDate),
      end: startOfDay(values.endDate),
    };
  }
  return {
    start: combine(values.startDate, values.startTime),
    end: combine(values.endDate, values.endTime),
  };
}

export function eventFormToApiInput(values: EventFormValues): ApiEventInput {
  const { start, end } = toDateRange(values);
  const description = values.description.trim();
  return {
    name: values.name,
    description: description ? description : null,
    notifications: values.notifications,
    color: values.color.toUpperCase(),
    is_all_day: values.isAllDay,
    start_at: toApiDateTime(start),
    end_at: toApiDateTime(end),
    discord_scheduled_event: values.discordEvent,
  };
}

/**
 * 既存の予定を編集フォームに読み込む。
 * 複製 (#91) もここを通るので、連携済み予定を複製するとチェックが入った状態で始まり、
 * 作成時に新しい Discord イベントも作られる
 */
export function eventToFormValues(event: ApiEvent): EventFormValues {
  const start = parseApiDateTime(event.start_at);
  const end = parseApiDateTime(event.end_at);
  return {
    name: event.name,
    description: event.description ?? "",
    color: event.color,
    isAllDay: event.is_all_day,
    startDate: startOfDay(start),
    startTime: format(start, "HH:mm"),
    endDate: startOfDay(end),
    endTime: format(end, "HH:mm"),
    notifications: event.notifications.map(({ num, unit }) => ({ num, unit })),
    discordEvent: event.discord_scheduled_event_id !== null,
  };
}

/**
 * カレンダー上で範囲選択したときの初期値。
 * end は FullCalendar 流儀の「含まない」(終日なら翌日 0:00) なので、終日は 1 日戻す
 */
export function newEventFormValues(
  start: Date,
  end: Date | null,
  allDay: boolean,
): EventFormValues {
  const base = {
    name: "",
    description: "",
    color: DEFAULT_COLOR,
    notifications: DEFAULT_NOTIFICATIONS.map((n) => ({ ...n })),
    isAllDay: allDay,
    discordEvent: false,
  };
  if (allDay) {
    const first = startOfDay(start);
    const last = end ? subDays(startOfDay(end), 1) : first;
    // 「終日」を外したときのために、時刻は旧フォームと同じ既定 (今の HH:00 〜 HH:30) を入れておく
    const now = startOfHour(new Date());
    return {
      ...base,
      startDate: first,
      endDate: max([first, last]),
      startTime: format(now, "HH:mm"),
      endTime: format(addMinutes(now, 30), "HH:mm"),
    };
  }
  const endAt = end ?? addHours(start, 1);
  return {
    ...base,
    startDate: startOfDay(start),
    startTime: format(start, "HH:mm"),
    endDate: startOfDay(endAt),
    endTime: format(endAt, "HH:mm"),
  };
}

/** 「新規作成」ボタンからの既定値 (旧フォーム: 今日の HH:00 〜 HH:30) */
export function defaultEventFormValues(now = new Date()): EventFormValues {
  const start = startOfHour(now);
  return newEventFormValues(start, addMinutes(start, 30), false);
}
