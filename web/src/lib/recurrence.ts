import type { RecurrenceRule } from "@/lib/api/types";
import { formatDisplayDate, type Language } from "@/lib/i18n";
export const WEEKDAYS = ["月", "火", "水", "木", "金", "土", "日"];
export function recurrenceWeekdays(language: Language): string[] {
  return language === "en"
    ? ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    : WEEKDAYS;
}
export function describeRecurrence(
  rule: RecurrenceRule,
  language: Language = "ja",
): string {
  if (language === "en") {
    if (rule.frequency === "none") return "Does not repeat";
    const days = recurrenceWeekdays(language);
    const frequency =
      rule.frequency === "daily"
        ? "Daily"
        : rule.frequency === "weekly" || rule.frequency === "biweekly"
          ? `${rule.frequency === "weekly" ? "Weekly" : "Every 2 weeks"} on ${rule.weekdays.map((day) => days[day]).join(", ")}`
          : rule.frequency === "monthly_date"
            ? `Monthly on day ${rule.day}`
            : `Monthly on ${rule.nth === 1 ? "1st" : rule.nth === 2 ? "2nd" : rule.nth === 3 ? "3rd" : `${rule.nth}th`} ${days[rule.weekday]}`;
    const ending =
      rule.end.type === "never"
        ? "no end date"
        : rule.end.type === "count"
          ? `${rule.end.count} occurrence${rule.end.count === 1 ? "" : "s"}`
          : `until ${formatDisplayDate(new Date(`${rule.end.date}T00:00:00`), language)}`;
    return `${frequency} / ${ending}`;
  }
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
