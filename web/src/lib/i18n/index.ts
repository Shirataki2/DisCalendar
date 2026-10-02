import { english } from "./messages";

export type Language = "ja" | "en";
export const LANGUAGE_STORAGE_KEY = "discalendar-language";

/** 地域付きの言語も受け付け、未対応言語・壊れた保存値は日本語にする。 */
export function resolveLanguage(value: unknown): Language {
  return typeof value === "string" && /^en(?:-|$)/i.test(value) ? "en" : "ja";
}

export type MessageValues = Record<string, string | number>;

/** キーは日本語の案内そのもの。未定義の英訳でも生の識別子は表示しない。 */
export function translate(
  language: Language,
  message: string,
  values: MessageValues = {},
): string {
  const text =
    language === "en" && Object.hasOwn(english, message)
      ? english[message]
      : message;
  return text.replace(/\{(\w+)\}/g, (placeholder, key: string) =>
    Object.hasOwn(values, key) ? String(values[key]) : placeholder,
  );
}

/** JST の壁時計として組み立てた Date を、タイムゾーンを変えずに表記する。 */
export function formatDisplayDate(
  date: Date,
  language: Language,
  options: Intl.DateTimeFormatOptions = {},
): string {
  const wallClock = new Date(0);
  wallClock.setUTCFullYear(date.getFullYear(), date.getMonth(), date.getDate());
  wallClock.setUTCHours(
    date.getHours(),
    date.getMinutes(),
    date.getSeconds(),
    0,
  );
  return new Intl.DateTimeFormat(language === "en" ? "en-US" : "ja-JP", {
    year: "numeric",
    month: "short",
    day: "numeric",
    ...options,
    timeZone: "UTC",
  }).format(wallClock);
}
