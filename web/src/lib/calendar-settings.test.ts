import { describe, expect, it } from "vitest";
import {
  DEFAULT_CALENDAR_SETTINGS,
  parseCalendarSettings,
  parseCalendarView,
  resolveInitialView,
} from "@/lib/calendar-settings";

describe("parseCalendarSettings", () => {
  it("保存が無ければ既定値 (月表示・日曜始まり) を返す", () => {
    expect(parseCalendarSettings(null)).toEqual({
      initialView: "dayGridMonth",
      firstDay: 0,
    });
  });

  it("保存された設定をそのまま返す", () => {
    expect(
      parseCalendarSettings(
        JSON.stringify({ initialView: "timeGridWeek", firstDay: 1 }),
      ),
    ).toEqual({ initialView: "timeGridWeek", firstDay: 1 });
  });

  it("「前回開いていたビュー」(last) も設定値として受け付ける", () => {
    expect(
      parseCalendarSettings(JSON.stringify({ initialView: "last" })),
    ).toEqual({ initialView: "last", firstDay: 0 });
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
    ).toEqual({ initialView: "dayGridMonth", firstDay: 1 });
    expect(
      parseCalendarSettings(
        JSON.stringify({ initialView: "listMonth", firstDay: 6 }),
      ),
    ).toEqual({ initialView: "listMonth", firstDay: 0 });
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
      resolveInitialView({ initialView: "timeGridDay", firstDay: 0 }, null),
    ).toBe("timeGridDay");
    // 明示されていれば前回のビューの記録は見ない
    expect(
      resolveInitialView(
        { initialView: "timeGridDay", firstDay: 0 },
        "listMonth",
      ),
    ).toBe("timeGridDay");
  });

  it("「前回開いていたビュー」は記録があればそれ、無ければ月にする", () => {
    expect(
      resolveInitialView({ initialView: "last", firstDay: 0 }, "listMonth"),
    ).toBe("listMonth");
    expect(resolveInitialView({ initialView: "last", firstDay: 0 }, null)).toBe(
      "dayGridMonth",
    );
  });
});
