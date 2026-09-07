import { describe, expect, it, vi } from "vitest";
import { notificationUrl, openPush, showPush } from "./push-worker";

const origin = "https://discalendar.app";
const url = `${origin}/dashboard/123?date=2026-09-07`;
function fixture(tabs: unknown[] = []) {
  const showNotification = vi.fn().mockResolvedValue(undefined);
  const openWindow = vi.fn().mockResolvedValue(null);
  const worker = {
    location: { origin },
    registration: { showNotification },
    clients: { matchAll: vi.fn().mockResolvedValue(tabs), openWindow },
  } as unknown as ServiceWorkerGlobalScope;
  return { worker, showNotification, openWindow };
}
describe("プッシュ通知", () => {
  it("タイトル・日時・サーバーを表示し、不正なペイロードも表示する", async () => {
    const { worker, showNotification } = fixture();
    await showPush(worker, {
      json: () => ({
        title: "定例会",
        body: "サーバー\n09/07 20:00 (JST)",
        url,
        tag: "event-1",
      }),
    } as PushMessageData);
    expect(showNotification).toHaveBeenCalledWith(
      "定例会",
      expect.objectContaining({
        body: "サーバー\n09/07 20:00 (JST)",
        data: { url },
        tag: "event-1",
      }),
    );
    await showPush(worker, {
      json: () => {
        throw new Error();
      },
    } as unknown as PushMessageData);
    expect(showNotification).toHaveBeenLastCalledWith(
      "DisCalendar",
      expect.objectContaining({ data: { url: `${origin}/dashboard` } }),
    );
  });
  it("外部 URL とカレンダー以外には遷移しない", () => {
    for (const bad of [
      "https://evil.example/dashboard/123?date=2026-09-07",
      "javascript:alert(1)",
      "/admin",
      null,
    ])
      expect(notificationUrl(bad, origin)).toBe(`${origin}/dashboard`);
    expect(notificationUrl(url, origin)).toBe(url);
  });
  it("開いているタブを該当日に移動して前面に出す", async () => {
    const focus = vi.fn();
    const navigate = vi.fn().mockResolvedValue({ focus });
    const { worker, openWindow } = fixture([
      { url: `${origin}/dashboard`, navigate },
    ]);
    const close = vi.fn();
    await openPush(worker, { data: { url }, close } as unknown as Notification);
    expect(navigate).toHaveBeenCalledWith(url);
    expect(focus).toHaveBeenCalledOnce();
    expect(close).toHaveBeenCalledOnce();
    expect(openWindow).not.toHaveBeenCalled();
  });
  it("タブがなければ開く", async () => {
    const { worker, openWindow } = fixture();
    await openPush(worker, {
      data: { url },
      close: vi.fn(),
    } as unknown as Notification);
    expect(openWindow).toHaveBeenCalledWith(url);
  });
});
