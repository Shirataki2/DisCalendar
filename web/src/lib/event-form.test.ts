import { describe, expect, it } from "vitest";
import type { ApiEvent } from "@/lib/api/types";
import {
  DEFAULT_COLOR,
  DESCRIPTION_MAX_CHARS,
  defaultEventFormValues,
  type EventFormValues,
  eventFormSchema,
  eventFormToApiInput,
  eventToFormValues,
  NAME_MAX_CHARS,
  NOTIFICATION_NUM_MAX,
  NOTIFICATIONS_MAX,
  newEventFormValues,
  toDateRange,
  withCheckedDiscordEvent,
} from "@/lib/event-form";

// 日付はローカル時刻で組み立てる (API の JST 文字列はブラウザのローカル時刻をそのまま JST とみなすので、
// テストの実行環境のタイムゾーンに依存しない)
const day = (d: number, h = 0, m = 0) => new Date(2026, 7, d, h, m);

const valid: EventFormValues = {
  name: "定例会",
  isAllDay: false,
  startDate: day(23),
  startTime: "10:00",
  endDate: day(23),
  endTime: "11:30",
  color: "#f44336",
  notifications: [{ num: 1, unit: "days" }],
  description: "",
  discordEvent: false,
};

/** 検証エラーを path ("a.b") → message の一覧にする */
function issuesOf(values: unknown): Record<string, string> {
  const result = eventFormSchema.safeParse(values);
  if (result.success) return {};
  return Object.fromEntries(
    result.error.issues.map((issue) => [issue.path.join("."), issue.message]),
  );
}

describe("eventFormSchema", () => {
  it("正しい入力を受け付ける", () => {
    expect(eventFormSchema.safeParse(valid).success).toBe(true);
  });

  it("タイトルは必須で、前後の空白を除いて上限 (api の validate と同じ) まで", () => {
    expect(issuesOf({ ...valid, name: "   " })).toHaveProperty("name");
    expect(issuesOf({ ...valid, name: "あ".repeat(NAME_MAX_CHARS) })).toEqual(
      {},
    );
    expect(
      issuesOf({ ...valid, name: "あ".repeat(NAME_MAX_CHARS + 1) }),
    ).toHaveProperty("name");
    // trim されるので空白込みで上限を超えていてもよい
    const parsed = eventFormSchema.parse({
      ...valid,
      name: `  ${"あ".repeat(NAME_MAX_CHARS)}  `,
    });
    expect(parsed.name).toBe("あ".repeat(NAME_MAX_CHARS));
  });

  it("説明の上限", () => {
    expect(
      issuesOf({ ...valid, description: "a".repeat(DESCRIPTION_MAX_CHARS) }),
    ).toEqual({});
    expect(
      issuesOf({
        ...valid,
        description: "a".repeat(DESCRIPTION_MAX_CHARS + 1),
      }),
    ).toHaveProperty("description");
  });

  it("色は #RRGGBB のみ", () => {
    expect(issuesOf({ ...valid, color: "#ABCDEF" })).toEqual({});
    expect(issuesOf({ ...valid, color: "#fff" })).toHaveProperty("color");
    expect(issuesOf({ ...valid, color: "red" })).toHaveProperty("color");
  });

  it("通知は件数と num の範囲を検証する", () => {
    const many = Array.from({ length: NOTIFICATIONS_MAX }, () => ({
      num: 5,
      unit: "minutes" as const,
    }));
    expect(issuesOf({ ...valid, notifications: many })).toEqual({});
    expect(
      issuesOf({ ...valid, notifications: [...many, many[0]] }),
    ).toHaveProperty("notifications");
    expect(
      issuesOf({ ...valid, notifications: [{ num: 0, unit: "hours" }] }),
    ).toHaveProperty("notifications.0.num");
    expect(
      issuesOf({
        ...valid,
        notifications: [{ num: NOTIFICATION_NUM_MAX + 1, unit: "hours" }],
      }),
    ).toHaveProperty("notifications.0.num");
    expect(
      issuesOf({ ...valid, notifications: [{ num: 1.5, unit: "hours" }] }),
    ).toHaveProperty("notifications.0.num");
    expect(
      issuesOf({ ...valid, notifications: [{ num: 1, unit: "months" }] }),
    ).toHaveProperty("notifications.0.unit");
  });

  it("時間指定のときは HH:mm の時刻が必須", () => {
    expect(issuesOf({ ...valid, startTime: "" })).toHaveProperty("startTime");
    expect(issuesOf({ ...valid, endTime: "25:00" })).toHaveProperty("endTime");
    expect(issuesOf({ ...valid, endTime: "9:00" })).toHaveProperty("endTime");
    // 終日なら時刻欄は見ない
    expect(
      issuesOf({ ...valid, isAllDay: true, startTime: "", endTime: "" }),
    ).toEqual({});
  });

  it("終了が開始より前ならエラー (時間指定は endTime、終日は endDate に付く)", () => {
    expect(issuesOf({ ...valid, endTime: "09:59" })).toEqual({
      endTime: "終了日時を開始日時より前にすることはできません",
    });
    // 同時刻は許可 (isBefore は厳密)
    expect(issuesOf({ ...valid, endTime: "10:00" })).toEqual({});
    // ただし Discord 連携 (#94) を有効にした時刻指定の予定は、終了が開始より後でないと
    // Discord 側が受け付けない (api の validate_discord_flag と同じ条件)
    expect(
      issuesOf({ ...valid, endTime: "10:00", discordEvent: true }),
    ).toHaveProperty("endTime");
    // 終日は終了に +1 日されるので同日でよい
    expect(issuesOf({ ...valid, isAllDay: true, discordEvent: true })).toEqual(
      {},
    );
    expect(
      issuesOf({ ...valid, isAllDay: true, endDate: day(22) }),
    ).toHaveProperty("endDate");
    // 終日は日付だけ見る (時刻欄の値が逆でもよい)
    expect(
      issuesOf({
        ...valid,
        isAllDay: true,
        endDate: day(23),
        startTime: "12:00",
        endTime: "08:00",
      }),
    ).toEqual({});
  });
});

describe("withCheckedDiscordEvent", () => {
  // 予定の開始は 2026-08-23 10:00 (ローカル = JST とみなす)。
  // 判定に渡す「今」は UTC で作る (nowInJst が JST の壁時計に読み替える)
  const utc = (d: number, h: number, m = 0) =>
    new Date(Date.UTC(2026, 7, d, h, m));
  const linked: EventFormValues = { ...valid, discordEvent: true };

  it("開始が未来ならそのまま (同じ参照を返す)", () => {
    // UTC 00:59 = JST 09:59
    expect(withCheckedDiscordEvent(linked, utc(23, 0, 59))).toBe(linked);
  });

  it("開始時刻をまたいでいたらチェックを落とす", () => {
    // UTC 01:00 = JST 10:00 (開始と同時刻。api も「現在以前」は拒否する)
    expect(withCheckedDiscordEvent(linked, utc(23, 1)).discordEvent).toBe(
      false,
    );
    expect(withCheckedDiscordEvent(linked, utc(23, 2)).discordEvent).toBe(
      false,
    );
  });

  it("元から連携なしなら触らない", () => {
    expect(withCheckedDiscordEvent(valid, utc(23, 2))).toBe(valid);
  });

  it("時刻が不正で開始が決まらないときは触らない (フォームの検証に任せる)", () => {
    const broken = { ...linked, startTime: "" };
    expect(withCheckedDiscordEvent(broken, utc(23, 2))).toBe(broken);
  });
});

describe("toDateRange", () => {
  it("時間指定は日付と時刻を合わせる", () => {
    expect(toDateRange(valid)).toEqual({
      start: day(23, 10, 0),
      end: day(23, 11, 30),
    });
  });

  it("終日は両端とも 0:00 (終了日は含む)", () => {
    expect(
      toDateRange({
        isAllDay: true,
        startDate: day(23, 15),
        startTime: "10:00",
        endDate: day(24, 3),
        endTime: "11:00",
      }),
    ).toEqual({ start: day(23), end: day(24) });
  });
});

describe("eventFormToApiInput", () => {
  it("API の形式 (JST 文字列・大文字の色・空の説明は null) に変換する", () => {
    expect(eventFormToApiInput(valid)).toEqual({
      name: "定例会",
      description: null,
      notifications: [{ num: 1, unit: "days" }],
      color: "#F44336",
      is_all_day: false,
      start_at: "2026-08-23T10:00:00",
      end_at: "2026-08-23T11:30:00",
      discord_scheduled_event: false,
    });
  });

  it("Discord 連携のチェックはフラグとして送る", () => {
    expect(
      eventFormToApiInput({ ...valid, discordEvent: true })
        .discord_scheduled_event,
    ).toBe(true);
  });

  it("説明は前後の空白を除く", () => {
    expect(
      eventFormToApiInput({ ...valid, description: "  メモ\n " }).description,
    ).toBe("メモ");
  });

  it("終日予定は 0:00 固定で終了日を含む形 (DB と同じ表現) にする", () => {
    const input = eventFormToApiInput({
      ...valid,
      isAllDay: true,
      endDate: day(25),
    });
    expect(input.is_all_day).toBe(true);
    expect(input.start_at).toBe("2026-08-23T00:00:00");
    expect(input.end_at).toBe("2026-08-25T00:00:00");
  });
});

const apiEvent: ApiEvent = {
  id: 1,
  guild_id: "200000000000000001",
  name: "定例会",
  description: null,
  notifications: [{ num: 30, unit: "minutes" }],
  color: "#2196F3",
  is_all_day: false,
  start_at: "2026-08-23T10:00:00",
  end_at: "2026-08-24T11:30:00",
  created_at: "2026-08-01T00:00:00",
  created_by: null,
  updated_by: null,
  updated_at: null,
  discord_scheduled_event_id: null,
};

describe("eventToFormValues", () => {
  it("API の予定を編集フォームの値にする", () => {
    expect(eventToFormValues(apiEvent)).toEqual({
      name: "定例会",
      description: "",
      color: "#2196F3",
      isAllDay: false,
      startDate: day(23),
      startTime: "10:00",
      endDate: day(24),
      endTime: "11:30",
      notifications: [{ num: 30, unit: "minutes" }],
      discordEvent: false,
    });
  });

  it("Discord 連携済みの予定はチェックが入った状態で読み込む", () => {
    expect(
      eventToFormValues({
        ...apiEvent,
        discord_scheduled_event_id: "9001",
      }).discordEvent,
    ).toBe(true);
  });

  it("フォーム → API → フォームで値が保たれる", () => {
    const values: EventFormValues = {
      ...valid,
      description: "メモ",
      color: "#2196F3",
    };
    const input = eventFormToApiInput(values);
    expect(
      eventToFormValues({ ...apiEvent, ...input, description: "メモ" }),
    ).toEqual(values);
  });
});

describe("newEventFormValues", () => {
  it("時間指定の範囲選択は開始・終了をそのまま使う", () => {
    const values = newEventFormValues(day(23, 9, 30), day(23, 12, 0), false);
    expect(values).toMatchObject({
      isAllDay: false,
      startDate: day(23),
      startTime: "09:30",
      endDate: day(23),
      endTime: "12:00",
      name: "",
      color: DEFAULT_COLOR,
    });
    // 旧フォームの既定の通知 (1 日前と 1 時間前)
    expect(values.notifications).toEqual([
      { num: 1, unit: "days" },
      { num: 1, unit: "hours" },
    ]);
  });

  it("終了がなければ 1 時間後", () => {
    expect(newEventFormValues(day(23, 23, 30), null, false)).toMatchObject({
      startDate: day(23),
      startTime: "23:30",
      endDate: day(24),
      endTime: "00:30",
    });
  });

  it("終日の範囲選択は FullCalendar の「翌日 0:00 (含まない)」を 1 日戻す", () => {
    expect(newEventFormValues(day(23), day(26), true)).toMatchObject({
      isAllDay: true,
      startDate: day(23),
      endDate: day(25),
    });
    // 単日 (end が翌日) と end なし
    expect(newEventFormValues(day(23), day(24), true)).toMatchObject({
      startDate: day(23),
      endDate: day(23),
    });
    expect(newEventFormValues(day(23), null, true)).toMatchObject({
      startDate: day(23),
      endDate: day(23),
    });
  });

  it("終日でも「終日」を外したときのために HH:mm の時刻が入っている", () => {
    const values = newEventFormValues(day(23), day(24), true);
    expect(values.startTime).toMatch(/^\d{2}:00$/);
    expect(values.endTime).toMatch(/^\d{2}:30$/);
  });
});

describe("defaultEventFormValues", () => {
  it("今の HH:00 〜 HH:30 (旧フォームと同じ既定値)", () => {
    expect(defaultEventFormValues(day(23, 14, 47))).toMatchObject({
      isAllDay: false,
      startDate: day(23),
      startTime: "14:00",
      endDate: day(23),
      endTime: "14:30",
    });
  });
});
