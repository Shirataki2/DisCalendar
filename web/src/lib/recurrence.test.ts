import { expect, test } from "vitest";
import type { RecurrenceRule } from "@/lib/api/types";
import { describeRecurrence, normalizeRecurrenceRule } from "./recurrence";

test("毎年の条件を日本語と英語で表示する", () => {
  expect(
    describeRecurrence({ frequency: "yearly", end: { type: "never" } }),
  ).toBe("毎年／終了なし");
  expect(
    describeRecurrence(
      { frequency: "yearly", end: { type: "count", count: 3 } },
      "en",
    ),
  ).toBe("Yearly / 3 occurrences");
  expect(
    describeRecurrence({
      frequency: "yearly",
      end: { type: "until", date: "2032-02-29" },
    }),
  ).toBe("毎年／2032-02-29まで");
});

test("毎月の複数の日付・第n曜日を日本語と英語で表示する", () => {
  expect(
    describeRecurrence({
      frequency: "monthly_date",
      days: [25, 5, 15],
      end: { type: "never" },
    }),
  ).toBe("毎月5・15・25日／終了なし");
  expect(
    describeRecurrence(
      { frequency: "monthly_date", days: [5], end: { type: "never" } },
      "en",
    ),
  ).toBe("Monthly on day 5 / no end date");
  expect(
    describeRecurrence(
      { frequency: "monthly_date", days: [5, 15], end: { type: "never" } },
      "en",
    ),
  ).toBe("Monthly on days 5, 15 / no end date");
  const sundays: RecurrenceRule = {
    frequency: "monthly_weekday",
    nths: [1, 2, 3],
    weekdays: [6],
    end: { type: "count", count: 9 },
  };
  expect(describeRecurrence(sundays)).toBe("毎月第1・第2・第3日曜日／全9回");
  expect(describeRecurrence(sundays, "en")).toBe(
    "Monthly on the 1st, 2nd, 3rd Sun / 9 occurrences",
  );
  expect(
    describeRecurrence({
      frequency: "monthly_weekday",
      nths: [4, 2],
      weekdays: [6, 5],
      end: { type: "never" },
    }),
  ).toBe("毎月第2・第4土・日曜日／終了なし");
});

test("旧形式の毎月の条件を配列の形式にそろえる", () => {
  expect(
    normalizeRecurrenceRule({
      frequency: "monthly_date",
      day: 5,
      end: { type: "never" },
    }),
  ).toEqual({ frequency: "monthly_date", days: [5], end: { type: "never" } });
  expect(
    normalizeRecurrenceRule({
      frequency: "monthly_weekday",
      nth: 2,
      weekday: 6,
      end: { type: "never" },
    }),
  ).toEqual({
    frequency: "monthly_weekday",
    nths: [2],
    weekdays: [6],
    end: { type: "never" },
  });
  const current = { frequency: "monthly_date", days: [5, 15] };
  expect(normalizeRecurrenceRule(current)).toBe(current);
  expect(normalizeRecurrenceRule(null)).toBeNull();
});
