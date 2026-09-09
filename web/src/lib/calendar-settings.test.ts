import { describe, expect, it } from "vitest";
import {
  DEFAULT_CALENDAR_SETTINGS,
  parseCalendarSettings,
  parseCalendarView,
  resolveInitialView,
} from "@/lib/calendar-settings";

describe("parseCalendarSettings", () => {
  it("保存が無ければ既定値 (月表示・日曜始まり) を返す", () => {
    expect(parseCalendarSettings(null)).toMatchObject({
      initialView: "dayGridMonth",
      firstDay: 0,
    });
  });

  it("保存された設定をそのまま返す", () => {
    expect(
      parseCalendarSettings(
        JSON.stringify({ initialView: "timeGridWeek", firstDay: 1 }),
      ),
    ).toMatchObject({ initialView: "timeGridWeek", firstDay: 1 });
  });

  it("「前回開いていたビュー」(last) も設定値として受け付ける", () => {
    expect(
      parseCalendarSettings(JSON.stringify({ initialView: "last" })),
    ).toMatchObject({ initialView: "last", firstDay: 0 });
  });

  it("JSON として壊れた値は既定値に落とす", () => {
    expect(parseCalendarSettings("{oops")).toEqual(DEFAULT_CALENDAR_SETTINGS);
  });

  it("オブジェクトでない JSON は既定値に落とす", () => {
    expect(parseCalendarSettings("42")).toEqual(DEFAULT_CALENDAR_SETTINGS);
    expect(parseCalendarSettings("null")).toEqual(DEFAULT_CALENDAR_SETTINGS);
    expect(parseCalendarSettings('"timeGridDay"')).toEqual(
      DEFAULT_CALENDAR_SETTINGS,
    );
  });

  it("知らない値は項目ごとに既定値へ落とし、妥当な項目は残す", () => {
    expect(
      parseCalendarSettings(
        JSON.stringify({ initialView: "timeGridYear", firstDay: 1 }),
      ),
    ).toMatchObject({ initialView: "dayGridMonth", firstDay: 1 });
    expect(
      parseCalendarSettings(
        JSON.stringify({ initialView: "listMonth", firstDay: 6 }),
      ),
    ).toMatchObject({ initialView: "listMonth", firstDay: 0 });
  });
});

describe("parseCalendarView", () => {
  it("ビュー名ならそのまま返す", () => {
    expect(parseCalendarView("listMonth")).toBe("listMonth");
  });

  it("ビュー名でなければ null (last は設定値でありビュー名ではない)", () => {
    expect(parseCalendarView(null)).toBeNull();
    expect(parseCalendarView("")).toBeNull();
    expect(parseCalendarView("last")).toBeNull();
    expect(parseCalendarView("timeGridYear")).toBeNull();
  });
});

describe("resolveInitialView", () => {
  it("ビューが明示されていればそれを使う", () => {
    expect(
      resolveInitialView(
        {
          ...DEFAULT_CALENDAR_SETTINGS,
          initialView: "timeGridDay",
          firstDay: 0,
        },
        null,
      ),
    ).toBe("timeGridDay");
    // 明示されていれば前回のビューの記録は見ない
    expect(
      resolveInitialView(
        {
          ...DEFAULT_CALENDAR_SETTINGS,
          initialView: "timeGridDay",
          firstDay: 0,
        },
        "listMonth",
      ),
    ).toBe("timeGridDay");
  });

  it("「前回開いていたビュー」は記録があればそれ、無ければ月にする", () => {
    expect(
      resolveInitialView(
        { ...DEFAULT_CALENDAR_SETTINGS, initialView: "last", firstDay: 0 },
        "listMonth",
      ),
    ).toBe("listMonth");
    expect(
      resolveInitialView(
        { ...DEFAULT_CALENDAR_SETTINGS, initialView: "last", firstDay: 0 },
        null,
      ),
    ).toBe("dayGridMonth");
  });
});

describe("新規作成の既定値", () => {
  it("旧設定には赤・30分・サーバー通知を補う", () => {
    expect(parseCalendarSettings('{"firstDay":1}')).toMatchObject({
      firstDay: 1,
      defaultColor: "#F44336",
      defaultDurationMinutes: 30,
      defaultNotifications: null,
    });
  });

  it.each(
    [[], Array.from({ length: 10 }, () => ({ num: 100, unit: "minutes" }))].map(
      (notifications) => ({ notifications }),
    ),
  )("通知0〜10件を保持する", ({ notifications }) => {
    expect(
      parseCalendarSettings(
        JSON.stringify({
          defaultColor: "#abcdef",
          defaultDurationMinutes: 180,
          defaultNotifications: notifications,
        }),
      ),
    ).toMatchObject({
      defaultColor: "#abcdef",
      defaultDurationMinutes: 180,
      defaultNotifications: notifications,
    });
  });

  it.each(
    [
      Array.from({ length: 11 }, () => ({ num: 1, unit: "days" })),
      [{ num: 0, unit: "days" }],
      [{ num: 101, unit: "days" }],
      [{ num: 1.5, unit: "days" }],
      [{ num: "1", unit: "days" }],
      [{ num: 1, unit: "seconds" }],
      "invalid",
    ].map((defaultNotifications) => ({ defaultNotifications })),
  )("不正な通知はサーバー設定に戻す: %j", ({ defaultNotifications }) => {
    expect(
      parseCalendarSettings(
        JSON.stringify({ defaultNotifications, firstDay: 1 }),
      ),
    ).toMatchObject({ firstDay: 1, defaultNotifications: null });
  });

  it("不正な色・長さだけを既定へ戻す", () => {
    expect(
      parseCalendarSettings(
        JSON.stringify({
          defaultColor: "red",
          defaultDurationMinutes: 45,
          defaultNotifications: [],
        }),
      ),
    ).toMatchObject({
      defaultColor: "#F44336",
      defaultDurationMinutes: 30,
      defaultNotifications: [],
    });
  });
});
