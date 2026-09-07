/// <reference lib="webworker" />

/** 通知の遷移先は自サイトのサーバーカレンダーに限定する。 */
export function notificationUrl(value: unknown, origin: string): string {
  if (typeof value !== "string") return `${origin}/dashboard`;
  try {
    const url = new URL(value, origin);
    if (
      url.origin === origin &&
      /^\/dashboard\/\d+\/?$/.test(url.pathname) &&
      /^\d{4}-\d{2}-\d{2}$/.test(url.searchParams.get("date") ?? "")
    )
      return url.href;
  } catch {
    /* 不正な通知でもサーバー一覧へ戻れるようにする */
  }
  return `${origin}/dashboard`;
}

export async function showPush(
  worker: ServiceWorkerGlobalScope,
  data: PushMessageData | null,
) {
  let payload: Record<string, unknown> = {};
  try {
    const value = data?.json();
    if (value && typeof value === "object") payload = value;
  } catch {
    /* 空の通知は共通文言で表示 */
  }
  await worker.registration.showNotification(
    typeof payload.title === "string" ? payload.title : "DisCalendar",
    {
      body:
        typeof payload.body === "string"
          ? payload.body
          : "予定のお知らせがあります。",
      icon: "/icons/icon-192.png",
      tag: typeof payload.tag === "string" ? payload.tag : undefined,
      data: { url: notificationUrl(payload.url, worker.location.origin) },
    },
  );
}

export async function openPush(
  worker: ServiceWorkerGlobalScope,
  notification: Notification,
) {
  notification.close();
  const url = notificationUrl(notification.data?.url, worker.location.origin);
  const clients = await worker.clients.matchAll({
    type: "window",
    includeUncontrolled: true,
  });
  const tab =
    clients.find((client) => client.url === url) ??
    clients.find(
      (client) => new URL(client.url).origin === worker.location.origin,
    );
  if (tab) {
    const navigated = tab.url === url ? tab : await tab.navigate(url);
    if (navigated) {
      await navigated.focus();
      return;
    }
  }
  await worker.clients.openWindow(url);
}
