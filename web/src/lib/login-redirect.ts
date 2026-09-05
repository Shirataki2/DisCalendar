/** Proxy からレイアウトへ渡す、ログイン後に戻るカレンダーのパス。 */
export const RETURN_TO_HEADER = "x-discalendar-return-to";

/** 外部 URL や管理画面を受け入れず、カレンダー内の戻り先だけを保持する。 */
export function dashboardReturnPath(value: string | null): string {
  if (!value) return "/dashboard";
  const match = value.match(/^\/dashboard(?:\/(?:all|[0-9]{1,20}))?$/);
  return match?.[0] === value ? value : "/dashboard";
}

export function loginUrl(returnTo: string | null): string {
  const path = dashboardReturnPath(returnTo);
  if (path === "/dashboard") return "/login";
  return `/login?${new URLSearchParams({ returnTo: path })}`;
}
