import { expect, test } from "vitest";
import { describeRecurrence } from "./recurrence";

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
