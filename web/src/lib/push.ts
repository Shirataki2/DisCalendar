import { ApiError, api } from "@/lib/api";

export const VAPID_PUBLIC_KEY = process.env.NEXT_PUBLIC_VAPID_PUBLIC_KEY ?? "";
export const PUSH_QUERY_KEY = ["push-settings"] as const;

export function pushSupported() {
  return (
    window.isSecureContext &&
    "serviceWorker" in navigator &&
    "PushManager" in window &&
    "Notification" in window
  );
}
export function decodePublicKey(key: string): Uint8Array<ArrayBuffer> {
  return Uint8Array.from(atob(key.replace(/-/g, "+").replace(/_/g, "/")), (c) =>
    c.charCodeAt(0),
  );
}

export async function subscribeDevice(deviceName: string) {
  // 許可ダイアログはクリックの処理から直接呼ぶ (Service Worker の待機より前)。
  const permission = await Notification.requestPermission();
  if (permission !== "granted")
    throw new Error(
      "通知が許可されていません。ブラウザのサイト設定を確認してください。",
    );
  const registration = await navigator.serviceWorker.getRegistration();
  if (!registration?.active)
    throw new Error(
      "通知の準備中です。ページを再読み込みしてからお試しください。",
    );
  let subscription = await registration.pushManager.getSubscription();
  // VAPID の鍵を変更した後は古い鍵の購読を引き継がない。
  if (
    subscription?.options.applicationServerKey &&
    String(new Uint8Array(subscription.options.applicationServerKey)) !==
      String(decodePublicKey(VAPID_PUBLIC_KEY))
  ) {
    await api.push.removeCurrent(subscription.endpoint);
    await subscription.unsubscribe();
    subscription = null;
  }
  const existing = subscription;
  subscription ??= await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: decodePublicKey(VAPID_PUBLIC_KEY),
  });
  const { endpoint, keys } = subscription.toJSON();
  try {
    if (!endpoint || !keys?.p256dh || !keys.auth)
      throw new Error("端末の購読情報を取得できませんでした。");
    await api.push.subscribe({
      endpoint,
      p256dh: keys.p256dh,
      auth: keys.auth,
      device_name: deviceName,
    });
  } catch (error) {
    if (existing && error instanceof ApiError && error.status === 409) {
      await subscription.unsubscribe();
      throw new Error(
        "以前のアカウントの購読を解除しました。もう一度「この端末で受け取る」を押してください。",
      );
    }
    if (!existing) await subscription.unsubscribe();
    throw error;
  }
}

/** 端末とサーバーの両方を解除してからログアウトする。期限切れでも端末側は止める。 */
export async function unsubscribeCurrentDevice() {
  if (!("serviceWorker" in navigator)) return;
  const registration = await navigator.serviceWorker.getRegistration();
  const subscription = await registration?.pushManager?.getSubscription();
  if (!subscription) return;
  try {
    await api.push.removeCurrent(subscription.endpoint);
  } catch (error) {
    if (!(error instanceof ApiError && error.status === 401)) throw error;
  }
  await subscription.unsubscribe();
}
