import { parseISO } from "date-fns";
import type { RecurrenceRule } from "@/lib/api/types";
import { formatDisplayDate, type Language } from "@/lib/i18n";
export const WEEKDAYS = ["月", "火", "水", "木", "金", "土", "日"];
export function recurrenceWeekdays(language: Language): string[] {
  return language === "en"
    ? ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    : WEEKDAYS;
}
/** 1st / 2nd / 3rd / 4th … (英語) または 第1 (日本語) */
export function ordinalWeek(nth: number, language: Language): string {
  if (language !== "en") return `第${nth}`;
  return nth === 1 ? "1st" : nth === 2 ? "2nd" : nth === 3 ? "3rd" : `${nth}th`;
}

/**
 * #355 以前の毎月の条件 (`day` / `nth` + `weekday`) を配列の形式にそろえる。
 * 変更履歴のスナップショットには旧形式が残るため、比較・表示の前に通す。
 */
export function normalizeRecurrenceRule(rule: unknown): unknown {
  if (!rule || typeof rule !== "object") return rule;
  const value = rule as Record<string, unknown>;
  if (value.frequency === "monthly_date" && "day" in value) {
    const { day, ...rest } = value;
    return { ...rest, days: [day] };
  }
  if (value.frequency === "monthly_weekday" && "nth" in value) {
    const { nth, weekday, ...rest } = value;
    return { ...rest, nths: [nth], weekdays: [weekday] };
  }
  return rule;
}

function sorted(values: number[]): number[] {
  return [...new Set(values)].sort((a, b) => a - b);
}

export function describeRecurrence(
  rule: RecurrenceRule,
  language: Language = "ja",
): string {
  if (language === "en") {
    if (rule.frequency === "none") return "Does not repeat";
    const days = recurrenceWeekdays(language);
    const frequency =
      rule.frequency === "yearly"
        ? "Yearly"
        : rule.frequency === "daily"
          ? "Daily"
          : rule.frequency === "weekly" || rule.frequency === "biweekly"
            ? `${rule.frequency === "weekly" ? "Weekly" : "Every 2 weeks"} on ${rule.weekdays.map((day) => days[day]).join(", ")}`
            : rule.frequency === "monthly_date"
              ? `Monthly on day${rule.days.length === 1 ? "" : "s"} ${sorted(rule.days).join(", ")}`
              : `Monthly on the ${sorted(rule.nths)
                  .map((nth) => ordinalWeek(nth, language))
                  .join(", ")} ${sorted(rule.weekdays)
                  .map((day) => days[day])
                  .join(", ")}`;
    const endDate = rule.end.type === "until" ? parseISO(rule.end.date) : null;
    const ending =
      rule.end.type === "never"
        ? "no end date"
        : rule.end.type === "count"
          ? `${rule.end.count} occurrence${rule.end.count === 1 ? "" : "s"}`
          : endDate && !Number.isNaN(endDate.getTime())
            ? `until ${formatDisplayDate(endDate, language)}`
            : "select a valid end date";
    return `${frequency} / ${ending}`;
  }
  if (rule.frequency === "none") return "繰り返しなし";
  const frequency =
    rule.frequency === "yearly"
      ? "毎年"
      : rule.frequency === "daily"
        ? "毎日"
        : rule.frequency === "weekly" || rule.frequency === "biweekly"
          ? `${rule.frequency === "weekly" ? "毎週" : "隔週"}${rule.weekdays.map((day) => WEEKDAYS[day]).join("・")}曜日`
          : rule.frequency === "monthly_date"
            ? `毎月${sorted(rule.days).join("・")}日`
            : `毎月${sorted(rule.nths)
                .map((nth) => ordinalWeek(nth, language))
                .join("・")}${sorted(rule.weekdays)
                .map((day) => WEEKDAYS[day])
                .join("・")}曜日`;
  const ending =
    rule.end.type === "never"
      ? "終了なし"
      : rule.end.type === "count"
        ? `全${rule.end.count}回`
        : `${rule.end.date}まで`;
  return `${frequency}／${ending}`;
}
