import { describe, expect, it } from "vitest";
import {
  DONATION_NOTICE_THRESHOLD,
  dismissDonationNotice,
  recordEventCreated,
  shouldShowDonationNotice,
} from "./donation-notice";

/** テスト用の最小限の Storage (Vitest は node 環境なので localStorage が無い) */
function memoryStorage(initial: Record<string, string> = {}): Storage {
  const data = new Map(Object.entries(initial));
  return {
    get length() {
      return data.size;
    },
    clear: () => data.clear(),
    getItem: (key) => data.get(key) ?? null,
    key: (index) => [...data.keys()][index] ?? null,
    removeItem: (key) => {
      data.delete(key);
    },
    setItem: (key, value) => {
      data.set(key, value);
    },
  };
}

/** 読み書きのたびに例外を投げる Storage (容量超過や保存がブロックされた環境) */
function throwingStorage(): Storage {
  const fail = () => {
    throw new Error("blocked");
  };
  return {
    length: 0,
    clear: fail,
    getItem: fail,
    key: fail,
    removeItem: fail,
    setItem: fail,
  };
}

describe("支援のお知らせの表示条件", () => {
  it("予定を作る前は出さない", () => {
    expect(shouldShowDonationNotice(memoryStorage())).toBe(false);
  });

  it("しきい値の件数を作るまでは出さず、達したら出す", () => {
    const storage = memoryStorage();
    for (let i = 1; i < DONATION_NOTICE_THRESHOLD; i++) {
      recordEventCreated(storage);
      expect(shouldShowDonationNotice(storage)).toBe(false);
    }
    recordEventCreated(storage);
    expect(shouldShowDonationNotice(storage)).toBe(true);
  });

  it("しきい値に達した後は数を増やさない", () => {
    const storage = memoryStorage();
    for (let i = 0; i < DONATION_NOTICE_THRESHOLD + 5; i++) {
      recordEventCreated(storage);
    }
    expect(storage.getItem("discalendar-created-event-count")).toBe(
      String(DONATION_NOTICE_THRESHOLD),
    );
  });

  it("閉じたら二度と出さない", () => {
    const storage = memoryStorage();
    for (let i = 0; i < DONATION_NOTICE_THRESHOLD; i++) {
      recordEventCreated(storage);
    }
    dismissDonationNotice(storage);
    expect(shouldShowDonationNotice(storage)).toBe(false);
    recordEventCreated(storage);
    expect(shouldShowDonationNotice(storage)).toBe(false);
  });

  it("壊れた保存値は 0 件として扱う", () => {
    const storage = memoryStorage({
      "discalendar-created-event-count": "abc",
    });
    expect(shouldShowDonationNotice(storage)).toBe(false);
    recordEventCreated(storage);
    expect(storage.getItem("discalendar-created-event-count")).toBe("1");
  });

  it("保存が使えない環境では出さず、例外も投げない", () => {
    const storage = throwingStorage();
    expect(() => recordEventCreated(storage)).not.toThrow();
    expect(() => dismissDonationNotice(storage)).not.toThrow();
    expect(shouldShowDonationNotice(storage)).toBe(false);
    expect(shouldShowDonationNotice(undefined)).toBe(false);
  });
});
