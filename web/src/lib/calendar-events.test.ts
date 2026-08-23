import type { EventApi } from "@fullcalendar/react";
import { describe, expect, it } from "vitest";
import type { ApiEvent } from "@/lib/api/types";
import {
  describeEventRange,
  describeNotification,
  parseApiDateTime,
  sourceOf,
  toApiDateTime,
  toApiEventInput,
  toApiRange,
  toCalendarEvent,
} from "@/lib/calendar-events";

const day = (d: number, h = 0, m = 0) => new Date(2026, 7, d, h, m);

const event: ApiEvent = {
  id: 42,
  guild_id: "200000000000000001",
  name: "定例会",
  description: "メモ",
  notifications: [{ num: 1, unit: "days" }],
  color: "#FFEB3B",
  is_all_day: false,
  start_at: "2026-08-23T10:00:00",
  end_at: "2026-08-23T11:30:00",
  created_at: "2026-08-01T00:00:00",
};

/** FullCalendar の EventApi のうち、変換に使う部分だけを持つ偽物 */
function fakeEventApi(fields: {
  start: Date | null;
  end: Date | null;
  allDay: boolean;
  source?: unknown;
}): EventApi {
  return {
    start: fields.start,
    end: fields.end,
    allDay: fields.allDay,
    extendedProps: { source: fields.source },
  } as unknown as EventApi;
}

describe("toApiDateTime / parseApiDateTime", () => {
  it("タイムゾーンなしの JST 文字列と相互変換する", () => {
    expect(toApiDateTime(day(23, 9, 5))).toBe("2026-08-23T09:05:00");
    expect(parseApiDateTime("2026-08-23T09:05:00")).toEqual(day(23, 9, 5));
  });
});

describe("toApiRange", () => {
  it("時間指定はそのまま", () => {
    expect(toApiRange(day(23, 10), day(23, 11, 30), false)).toEqual({
      start_at: "2026-08-23T10:00:00",
      end_at: "2026-08-23T11:30:00",
    });
  });

  it("時間指定で end がなければ 1 時間 (FullCalendar の既定表示と同じ)", () => {
    expect(toApiRange(day(23, 10), null, false)).toEqual({
      start_at: "2026-08-23T10:00:00",
      end_at: "2026-08-23T11:00:00",
    });
  });

  it("終日は FullCalendar の end (翌日 0:00、含まない) を DB の終了日 (含む) に 1 日戻す", () => {
    expect(toApiRange(day(23), day(26), true)).toEqual({
      start_at: "2026-08-23T00:00:00",
      end_at: "2026-08-25T00:00:00",
    });
  });

  it("終日で end がない / 同じ日なら単日", () => {
    expect(toApiRange(day(23), null, true).end_at).toBe("2026-08-23T00:00:00");
    expect(toApiRange(day(23), day(24), true).end_at).toBe(
      "2026-08-23T00:00:00",
    );
    // 時刻付きで渡されても 0:00 に丸める
    expect(toApiRange(day(23, 15), day(24, 3), true)).toEqual({
      start_at: "2026-08-23T00:00:00",
      end_at: "2026-08-23T00:00:00",
    });
  });
});

describe("toCalendarEvent", () => {
  it("時間指定の予定は API の文字列をそのまま使い、元の予定を extendedProps に持つ", () => {
    const input = toCalendarEvent(event);
    expect(input).toEqual({
      id: "42",
      title: "定例会",
      color: "#FFEB3B",
      // 黄色の背景には黒文字
      textColor: "#000000",
      extendedProps: { source: event },
      allDay: false,
      start: "2026-08-23T10:00:00",
      end: "2026-08-23T11:30:00",
    });
  });

  it("終日の予定は終了日を 1 日進めて日付だけにする", () => {
    const allDay: ApiEvent = {
      ...event,
      color: "#212121",
      is_all_day: true,
      start_at: "2026-08-23T00:00:00",
      end_at: "2026-08-25T00:00:00",
    };
    expect(toCalendarEvent(allDay)).toMatchObject({
      allDay: true,
      start: "2026-08-23",
      end: "2026-08-26",
      textColor: "#ffffff",
    });
    // 単日の終日予定
    expect(
      toCalendarEvent({ ...allDay, end_at: "2026-08-23T00:00:00" }),
    ).toMatchObject({ start: "2026-08-23", end: "2026-08-24" });
  });
});

describe("sourceOf", () => {
  it("toCalendarEvent で入れた元の予定を取り出す", () => {
    expect(
      sourceOf(
        fakeEventApi({
          start: day(23),
          end: null,
          allDay: false,
          source: event,
        }),
      ),
    ).toBe(event);
  });

  it("元の予定がなければ null", () => {
    expect(
      sourceOf(fakeEventApi({ start: day(23), end: null, allDay: false })),
    ).toBeNull();
    expect(
      sourceOf(
        fakeEventApi({
          start: day(23),
          end: null,
          allDay: false,
          source: { id: "42" },
        }),
      ),
    ).toBeNull();
  });
});

describe("toApiEventInput", () => {
  it("ドラッグ後の日時と終日フラグ以外は元の予定を引き継ぐ", () => {
    const moved = fakeEventApi({
      start: day(24, 13),
      end: day(24, 14, 30),
      allDay: false,
    });
    expect(toApiEventInput(moved, event)).toEqual({
      name: "定例会",
      description: "メモ",
      notifications: [{ num: 1, unit: "days" }],
      color: "#FFEB3B",
      is_all_day: false,
      start_at: "2026-08-24T13:00:00",
      end_at: "2026-08-24T14:30:00",
    });
  });

  it("終日へドラッグすると終日の表現 (終了日を含む) になる", () => {
    const moved = fakeEventApi({ start: day(24), end: null, allDay: true });
    expect(toApiEventInput(moved, event)).toMatchObject({
      is_all_day: true,
      start_at: "2026-08-24T00:00:00",
      end_at: "2026-08-24T00:00:00",
    });
  });

  it("start がなければ元の予定の開始日時を使う", () => {
    const moved = fakeEventApi({ start: null, end: null, allDay: false });
    expect(toApiEventInput(moved, event)).toMatchObject({
      start_at: "2026-08-23T10:00:00",
      end_at: "2026-08-23T11:00:00",
    });
  });
});

describe("describeNotification", () => {
  it("単位を日本語にする", () => {
    expect(describeNotification({ num: 30, unit: "minutes" })).toBe("30分前");
    expect(describeNotification({ num: 2, unit: "hours" })).toBe("2時間前");
    expect(describeNotification({ num: 1, unit: "days" })).toBe("1日前");
    expect(describeNotification({ num: 1, unit: "weeks" })).toBe("1週間前");
  });
});

describe("describeEventRange", () => {
  it("時間指定 (同日 / 別日)", () => {
    expect(describeEventRange(event)).toBe("2026/08/23 10:00 - 11:30");
    expect(
      describeEventRange({ ...event, end_at: "2026-08-24T09:00:00" }),
    ).toBe("2026/08/23 10:00 - 2026/08/24 09:00");
  });

  it("終日 (単日 / 複数日)", () => {
    const allDay: ApiEvent = {
      ...event,
      is_all_day: true,
      start_at: "2026-08-23T00:00:00",
      end_at: "2026-08-23T00:00:00",
    };
    expect(describeEventRange(allDay)).toBe("2026/08/23");
    expect(
      describeEventRange({ ...allDay, end_at: "2026-08-25T00:00:00" }),
    ).toBe("2026/08/23 - 2026/08/25");
  });
});
