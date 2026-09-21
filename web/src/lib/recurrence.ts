import type { RecurrenceRule } from "@/lib/api/types";
export const WEEKDAYS = ["月", "火", "水", "木", "金", "土", "日"];
export function describeRecurrence(rule: RecurrenceRule): string {
  if (rule.frequency === "none") return "繰り返しなし";
  const frequency =
    rule.frequency === "daily"
      ? "毎日"
      : rule.frequency === "weekly" || rule.frequency === "biweekly"
        ? `${rule.frequency === "weekly" ? "毎週" : "隔週"}${rule.weekdays.map((day) => WEEKDAYS[day]).join("・")}曜日`
        : rule.frequency === "monthly_date"
          ? `毎月${rule.day}日`
          : `毎月第${rule.nth}${WEEKDAYS[rule.weekday]}曜日`;
  const ending =
    rule.end.type === "never"
      ? "終了なし"
      : rule.end.type === "count"
        ? `全${rule.end.count}回`
        : `${rule.end.date}まで`;
  return `${frequency}／${ending}`;
}
