import { describe, expect, it } from "vitest";
import {
  formatChange,
  formatMonthDay,
  formatNaiveJst,
  formatPercent,
  formatYearMonth,
} from "@/lib/admin-format";

describe("formatMonthDay", () => {
  it("日付だけの文字列を月/日にする", () => {
    expect(formatMonthDay("2026-08-25")).toBe("8/25");
    expect(formatMonthDay("2026-01-01")).toBe("1/1");
  });

  it("ブラウザのタイムゾーンで日付をずらさない", () => {
    // new Date("2026-08-01") は UTC 0:00 と解釈されるので、JST 以外では 7/31 になってしまう。
    // 文字列のまま扱っているので、どのタイムゾーンでも 8/1 のまま
    expect(formatMonthDay("2026-08-01")).toBe("8/1");
  });

  it("想定しない形式はそのまま返す", () => {
    expect(formatMonthDay("2026-08")).toBe("2026-08");
  });
});

describe("formatYearMonth", () => {
  it("月初の日付を年/月にする", () => {
    expect(formatYearMonth("2026-08-01")).toBe("2026/08");
  });

  it("想定しない形式はそのまま返す", () => {
    expect(formatYearMonth("")).toBe("");
  });
});

describe("formatNaiveJst", () => {
  it("タイムゾーンなしの日時をそのまま整形する", () => {
    expect(formatNaiveJst("2026-08-25T09:05:00")).toBe("2026/08/25 09:05");
  });
});

describe("formatPercent", () => {
  it("割り切れるときは整数で出す", () => {
    expect(formatPercent(1, 4)).toBe("25%");
  });

  it("小数第 1 位まで丸める", () => {
    expect(formatPercent(1, 3)).toBe("33.3%");
  });

  it("分母が 0 なら計算せずに - を返す", () => {
    expect(formatPercent(0, 0)).toBe("-");
  });
});

describe("formatChange", () => {
  it("増加には + を付ける", () => {
    expect(formatChange(20)).toBe("+20%");
    expect(formatChange(33.33)).toBe("+33.3%");
  });

  it("減少と横ばいはそのまま出す", () => {
    expect(formatChange(-50)).toBe("-50%");
    expect(formatChange(0)).toBe("0%");
  });

  it("前の期間が 0 で計算できないとき (null) は - を返す", () => {
    expect(formatChange(null)).toBe("-");
  });
});
