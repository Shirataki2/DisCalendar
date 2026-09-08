import {
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
import {
  nowInJst,
  parseApiDateTime,
  toApiDateTime,
} from "@/lib/calendar-events";

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

/**
 * サーバー設定 (#181) を取れていないときの既定の事前通知 (旧フォームの既定 = 1 日前と 1 時間前)。
 * 通常はサーバー設定の `default_notifications` (api 側の既定値も同じ) を使う
 */
export const DEFAULT_NOTIFICATIONS: Notification[] = [
  { num: 1, unit: "days" },
  { num: 1, unit: "hours" },
];

const TIME_PATTERN = /^([01]\d|2[0-3]):[0-5]\d$/;
const HEX_COLOR_PATTERN = /^#[0-9A-Fa-f]{6}$/;
const NOTIFICATION_NUM_MESSAGE = `${NOTIFICATION_NUM_MIN}〜${NOTIFICATION_NUM_MAX}の範囲で入力してください`;

/** 「num unit 前」1 件。予定ダイアログとサーバー設定の「既定の事前通知」(#181) で共通 */
export const notificationSchema = z.object({
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

/** 色・通知の検証は予定と個人設定で共用する。 */
export const colorSchema = z
  .string()
  .regex(HEX_COLOR_PATTERN, "色は #RRGGBB 形式で指定してください");
export const notificationsSchema = z
  .array(notificationSchema)
  .max(NOTIFICATIONS_MAX, `通知は${NOTIFICATIONS_MAX}件まで設定できます`);
export const EVENT_DURATION_MINUTES = [30, 60, 90, 120, 180] as const;
export const eventCreationDefaultsSchema = z.object({
  defaultColor: colorSchema,
  defaultNotifications: notificationsSchema.nullable(),
  defaultDurationMinutes: z.union(
    EVENT_DURATION_MINUTES.map((minutes) => z.literal(minutes)),
  ),
});
export type EventCreationDefaults = z.infer<typeof eventCreationDefaultsSchema>;
export const DEFAULT_EVENT_CREATION_SETTINGS: EventCreationDefaults = {
  defaultColor: DEFAULT_COLOR,
  defaultNotifications: null,
  defaultDurationMinutes: 30,
};

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
    color: colorSchema,
    notifications: notificationsSchema,
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
    } else if (
      values.discordEvent &&
      !values.isAllDay &&
      end.getTime() === start.getTime()
    ) {
      // Discord の外部イベントは終了が開始より後である必要がある (api の validate_discord_flag と同じ条件)
      ctx.addIssue({
        code: "custom",
        path: ["endTime"],
        message: "Discord のイベントにするには終了を開始より後にしてください",
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
 * 送信直前に Discord 連携 (#94) の可否をもう一度確かめ、連携できない開始日時なら
 * チェックを落とした値を返す。
 *
 * チェックボックスの無効化は開いた時点の時刻で決まるので、ダイアログを開いたまま
 * 開始時刻をまたぐと、有効なまま送信されて api の検証 (`validate_discord_flag`) で
 * 400 になってしまう。案内どおり「過去開始なら連携しない (連携済みなら解除)」に倒す
 */
export function withCheckedDiscordEvent(
  values: EventFormValues,
  now = new Date(),
): EventFormValues {
  if (!values.discordEvent) return values;
  const startAt = formStartAt(values);
  if (startAt === null || startAt.getTime() > nowInJst(now).getTime()) {
    return values;
  }
  return { ...values, discordEvent: false };
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
 * end は FullCalendar 流儀の「含まない」(終日なら翌日 0:00) なので、終日は 1 日戻す。
 * `defaultNotifications` はサーバー設定の「新しい予定の既定の事前通知」(#181)。
 * 個人設定に通知の指定があれば優先し、両方未指定なら従来の既定を使う
 */
export function newEventFormValues(
  start: Date,
  end: Date | null,
  allDay: boolean,
  defaultNotifications: readonly Notification[] = DEFAULT_NOTIFICATIONS,
  defaults: EventCreationDefaults = DEFAULT_EVENT_CREATION_SETTINGS,
): EventFormValues {
  const base = {
    name: "",
    description: "",
    color: defaults.defaultColor,
    notifications: (defaults.defaultNotifications ?? defaultNotifications).map(
      ({ num, unit }) => ({ num, unit }),
    ),
    isAllDay: allDay,
    discordEvent: false,
  };
  if (allDay) {
    const first = startOfDay(start);
    const last = end ? subDays(startOfDay(end), 1) : first;
    // 終日の日付範囲は維持し、解除時に使う時刻だけ個人設定の長さにする
    const now = startOfHour(new Date());
    return {
      ...base,
      startDate: first,
      endDate: max([first, last]),
      startTime: format(now, "HH:mm"),
      endTime: format(
        addMinutes(now, defaults.defaultDurationMinutes),
        "HH:mm",
      ),
    };
  }
  const endAt = end ?? addMinutes(start, defaults.defaultDurationMinutes);
  return {
    ...base,
    startDate: startOfDay(start),
    startTime: format(start, "HH:mm"),
    endDate: startOfDay(endAt),
    endTime: format(endAt, "HH:mm"),
  };
}

/** 「新規作成」ボタンの初期値。今の正時から個人設定の長さ (未指定なら30分)。 */
export function defaultEventFormValues(
  now = new Date(),
  defaultNotifications?: readonly Notification[],
  defaults: EventCreationDefaults = DEFAULT_EVENT_CREATION_SETTINGS,
): EventFormValues {
  const start = startOfHour(now);
  return newEventFormValues(start, null, false, defaultNotifications, defaults);
}
