// ダッシュボードで一度だけ出す支援のお知らせ (#371) の表示条件。
//
// ある程度使ってくれている人にだけ出すため、「この端末のダッシュボードで予定を作った回数」を数え、
// DONATION_NOTICE_THRESHOLD 件に達したら次にカレンダーを開いたときに出す。閉じたら (支援ページを開いても) 二度と出さない。
// 保存先は最初の予定の案内 (components/first-event-guide.tsx) と同じく端末の localStorage にする。
// サーバーには何も送らない (支援の有無や表示の記録をアカウントに紐づけない) ので、端末を変えると数え直しになる。
// 練習用カレンダー (/tutorial) での作成は数えない (lib/query/events.ts の useCreateEvent)

/** お知らせを出すまでに作る予定の数 */
export const DONATION_NOTICE_THRESHOLD = 3;

const COUNT_KEY = "discalendar-created-event-count";
const DISMISSED_KEY = "discalendar-donation-notice-dismissed";

/** 予定の作成に成功したら呼ぶ。しきい値に達した後は数えない (保存の書き込みを減らすため) */
export function recordEventCreated(
  storage: Storage | undefined = safeStorage(),
) {
  if (!storage) return;
  try {
    const count = readCount(storage);
    if (count >= DONATION_NOTICE_THRESHOLD) return;
    storage.setItem(COUNT_KEY, String(count + 1));
  } catch {
    // 保存が使えない (プライベートブラウズなど) ときは数えない = お知らせを出さない
  }
}

/** お知らせを出すか。保存が使えないときは出さない (閉じた記録も残せず、毎回出てしまうため) */
export function shouldShowDonationNotice(
  storage: Storage | undefined = safeStorage(),
): boolean {
  if (!storage) return false;
  try {
    if (storage.getItem(DISMISSED_KEY) === "1") return false;
    return readCount(storage) >= DONATION_NOTICE_THRESHOLD;
  } catch {
    return false;
  }
}

/** 閉じた (または支援ページを開いた) ことを記録する */
export function dismissDonationNotice(
  storage: Storage | undefined = safeStorage(),
) {
  try {
    storage?.setItem(DISMISSED_KEY, "1");
  } catch {
    // 保存できなくても、このページでは閉じられるようにする (呼び出し側の state で消す)
  }
}

function readCount(storage: Storage): number {
  const count = Number.parseInt(storage.getItem(COUNT_KEY) ?? "0", 10);
  return Number.isFinite(count) && count > 0 ? count : 0;
}

function safeStorage(): Storage | undefined {
  try {
    return typeof window === "undefined" ? undefined : window.localStorage;
  } catch {
    // アクセス自体が例外になる環境 (Cookie をブロックしている Safari など)
    return undefined;
  }
}
