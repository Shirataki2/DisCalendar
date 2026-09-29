/** Proxy からレイアウトへ渡す、ログイン後に戻るカレンダーのパス。 */
export const RETURN_TO_HEADER = "x-discalendar-return-to";

export function calendarDateParam(
  value: string | string[] | null | undefined,
): string | undefined {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value))
    return undefined;
  const date = new Date(`${value}T00:00:00Z`);
  return !Number.isNaN(date.getTime()) &&
    date.toISOString().slice(0, 10) === value
    ? value
    : undefined;
}

export function calendarEventParam(
  value: string | string[] | null | undefined,
): number | undefined {
  if (typeof value !== "string" || !/^[1-9]\d*$/.test(value)) return undefined;
  const id = Number(value);
  return Number.isSafeInteger(id) && id <= 2_147_483_647 ? id : undefined;
}

/** 外部 URL や管理画面を受け入れず、カレンダー内の戻り先だけを保持する。 */
export function dashboardReturnPath(value: string | null): string {
  if (!value?.startsWith("/dashboard")) return "/dashboard";
  const url = new URL(value, "https://discalendar.invalid");
  if (
    url.origin !== "https://discalendar.invalid" ||
    !/^\/dashboard(?:\/(?:all|[0-9]{1,20}(?:\/polls(?:\/[1-9][0-9]{0,9})?)?))?$/.test(
      url.pathname,
    ) ||
    `${url.pathname}${url.search}` !== value
  )
    return "/dashboard";

  // 作成直後の古い共有URLも、案内表示だけのパラメーターを落として投票ページへ戻す。
  if (
    /\/polls\/[1-9][0-9]*$/.test(url.pathname) &&
    url.searchParams.has("announcement")
  ) {
    const values = url.searchParams.getAll("announcement");
    if (
      values.length !== 1 ||
      !["sent", "failed", "not_configured"].includes(values[0])
    )
      return "/dashboard";
    url.searchParams.delete("announcement");
  }
  for (const key of url.searchParams.keys()) {
    if (key !== "date" && key !== "event") return "/dashboard";
  }
  if (
    (url.searchParams.has("date") &&
      (url.searchParams.getAll("date").length !== 1 ||
        !calendarDateParam(url.searchParams.get("date")))) ||
    (url.searchParams.has("event") &&
      (url.searchParams.getAll("event").length !== 1 ||
        !calendarEventParam(url.searchParams.get("event"))))
  )
    return "/dashboard";
  return `${url.pathname}${url.search}`;
}

export function loginUrl(returnTo: string | null): string {
  const path = dashboardReturnPath(returnTo);
  if (path === "/dashboard") return "/login";
  return `/login?${new URLSearchParams({ returnTo: path })}`;
}
