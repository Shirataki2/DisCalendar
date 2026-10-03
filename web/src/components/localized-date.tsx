"use client";

import { useLanguage } from "@/components/language-provider";
import { describeEventRange } from "@/lib/calendar-events";

/** 絶対時刻は言語を切り替えても日本時間で表示する。 */
export function LocalizedDate({ value }: { value: string }) {
  const { language, t } = useLanguage();
  return (
    <>
      {new Intl.DateTimeFormat(language === "en" ? "en-US" : "ja-JP", {
        dateStyle: "medium",
        timeStyle: "short",
        timeZone: "Asia/Tokyo",
      }).format(new Date(value))}{" "}
      {t("(日本時間)")}
    </>
  );
}

export function LocalizedEventRange({
  event,
}: {
  event: Parameters<typeof describeEventRange>[0];
}) {
  const { language, t } = useLanguage();
  return (
    <>
      {describeEventRange(event, language)}{" "}
      {t(event.is_all_day ? "終日" : "(日本時間)")}
    </>
  );
}
