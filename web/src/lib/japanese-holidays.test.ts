import { describe, expect, it } from "vitest";
import { dayColorClass, holidayNameOf } from "@/lib/japanese-holidays";
import { JAPANESE_HOLIDAYS } from "@/lib/japanese-holidays.generated";

/**
 * FullCalendar の描画フックに渡る date を作る (実タイムゾーンの 0:00)。
 * タイムゾーン指定の無い ISO 文字列はローカル時刻として解釈されるので、
 * テストを動かす環境の TZ (CI の Node は UTC、手元は JST) によらず「その日」になる
 */
function cellDate(iso: string): Date {
  return new Date(`${iso}T00:00:00`);
}

describe("holidayNameOf", () => {
  it("祝日なら名称を返す", () => {
    expect(holidayNameOf(cellDate("2026-01-01"))).toBe("元日");
    expect(holidayNameOf(cellDate("2026-05-06"))).toBe("休日"); // 憲法記念日の振替
  });

  it("平日なら undefined を返す", () => {
    expect(holidayNameOf(cellDate("2026-08-30"))).toBeUndefined();
  });
});

describe("dayColorClass", () => {
  it("日曜・祝日は赤、土曜は青、平日は色なし", () => {
    // 2026-08-30 (日) / 2026-08-29 (土) / 2026-08-28 (金) / 2026-11-03 (火・文化の日)
    expect(dayColorClass({ date: cellDate("2026-08-30"), dow: 0 })).toBe(
      "cal-day-red",
    );
    expect(dayColorClass({ date: cellDate("2026-08-29"), dow: 6 })).toBe(
      "cal-day-blue",
    );
    expect(
      dayColorClass({ date: cellDate("2026-08-28"), dow: 5 }),
    ).toBeUndefined();
    expect(dayColorClass({ date: cellDate("2026-11-03"), dow: 2 })).toBe(
      "cal-day-red",
    );
  });

  it("土曜の祝日は赤が優先される", () => {
    // 土曜に当たる祝日は年によって変わるので、実データから探して検証する
    const saturdayHoliday = Object.keys(JAPANESE_HOLIDAYS).find(
      (date) => new Date(`${date}T00:00:00`).getDay() === 6,
    );
    expect(saturdayHoliday).toBeDefined();
    expect(
      dayColorClass({
        date: cellDate(saturdayHoliday as string),
        dow: 6,
      }),
    ).toBe("cal-day-red");
  });
});

describe("祝日データの鮮度", () => {
  it("今年いっぱいの祝日を含む (失敗したら pnpm holidays でデータを更新してコミットする)", () => {
    // 祝日は毎年 2 月ごろに翌年分が告示される。今年の分すら無いのは更新を 1 年以上
    // 忘れているということなので、CI を落として気づけるようにする (#97)
    const year = new Date().getFullYear();
    expect(JAPANESE_HOLIDAYS[`${year}-11-03`]).toBe("文化の日");
  });
});
