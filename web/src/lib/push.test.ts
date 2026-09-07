import { afterEach, expect, it, vi } from "vitest";
import { ApiError, api } from "@/lib/api";
import { subscribeDevice, unsubscribeCurrentDevice } from "./push";

vi.mock("@/lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/api")>()),
  api: { push: { removeCurrent: vi.fn(), subscribe: vi.fn() } },
}));
afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetAllMocks();
});

it("ログアウトする端末の購読をブラウザと API の両方で解除する", async () => {
  const unsubscribe = vi.fn().mockResolvedValue(true);
  vi.stubGlobal("navigator", {
    serviceWorker: {
      getRegistration: async () => ({
        pushManager: {
          getSubscription: async () => ({
            endpoint: "https://fcm.googleapis.com/test",
            unsubscribe,
          }),
        },
      }),
    },
  });
  await unsubscribeCurrentDevice();
  expect(unsubscribe).toHaveBeenCalledOnce();
  expect(api.push.removeCurrent).toHaveBeenCalledWith(
    "https://fcm.googleapis.com/test",
  );
});
it("セッションが期限切れでも端末側の購読は解除する", async () => {
  const unsubscribe = vi.fn().mockResolvedValue(true);
  vi.stubGlobal("navigator", {
    serviceWorker: {
      getRegistration: async () => ({
        pushManager: {
          getSubscription: async () => ({
            endpoint: "https://fcm.googleapis.com/test",
            unsubscribe,
          }),
        },
      }),
    },
  });
  vi.mocked(api.push.removeCurrent).mockRejectedValue(
    new ApiError(401, "unauthorized", "expired"),
  );
  await expect(unsubscribeCurrentDevice()).resolves.toBeUndefined();
  expect(unsubscribe).toHaveBeenCalledOnce();
});

it("API の解除が失敗しても endpoint を失わず次回に再試行する", async () => {
  const subscription = {
    endpoint: "https://fcm.googleapis.com/test",
    unsubscribe: vi.fn().mockResolvedValue(true),
  };
  vi.stubGlobal("navigator", {
    serviceWorker: {
      getRegistration: async () => ({
        pushManager: {
          getSubscription: async () =>
            subscription.unsubscribe.mock.calls.length ? null : subscription,
        },
      }),
    },
  });
  vi.mocked(api.push.removeCurrent)
    .mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValueOnce(undefined);
  await expect(unsubscribeCurrentDevice()).rejects.toThrow("offline");
  expect(subscription.unsubscribe).not.toHaveBeenCalled();
  await unsubscribeCurrentDevice();
  expect(api.push.removeCurrent).toHaveBeenCalledTimes(2);
  expect(subscription.unsubscribe).toHaveBeenCalledOnce();
});

it("別アカウントの購読が残っていても再登録の操作で復旧する", async () => {
  const old = {
    endpoint: "https://fcm.googleapis.com/old",
    options: {},
    unsubscribe: vi.fn().mockResolvedValue(true),
    toJSON: () => ({
      endpoint: "https://fcm.googleapis.com/old",
      keys: { p256dh: "key", auth: "auth" },
    }),
  };
  const fresh = {
    unsubscribe: vi.fn(),
    toJSON: () => ({
      endpoint: "https://fcm.googleapis.com/new",
      keys: { p256dh: "key", auth: "auth" },
    }),
  };
  const subscribe = vi.fn().mockResolvedValue(fresh);
  vi.stubGlobal("Notification", {
    requestPermission: vi.fn().mockResolvedValue("granted"),
  });
  vi.stubGlobal("navigator", {
    serviceWorker: {
      getRegistration: async () => ({
        active: {},
        pushManager: {
          getSubscription: async () =>
            old.unsubscribe.mock.calls.length ? null : old,
          subscribe,
        },
      }),
    },
  });
  vi.mocked(api.push.subscribe)
    .mockRejectedValueOnce(new ApiError(409, "conflict", "owner"))
    .mockResolvedValueOnce(undefined);
  await expect(subscribeDevice("端末")).rejects.toThrow(
    "以前のアカウントの購読を解除しました",
  );
  expect(old.unsubscribe).toHaveBeenCalledOnce();
  await subscribeDevice("端末");
  expect(subscribe).toHaveBeenCalledOnce();
  expect(api.push.subscribe).toHaveBeenLastCalledWith(
    expect.objectContaining({ endpoint: "https://fcm.googleapis.com/new" }),
  );
  expect(fresh.unsubscribe).not.toHaveBeenCalled();
});

it.each([new Error("offline"), new ApiError(503, "unavailable", "retry")])(
  "登録結果が不明な場合は新規購読を保持して同じ endpoint で再試行する (%s)",
  async (error) => {
    let current: unknown = null;
    const subscription = {
      options: {},
      unsubscribe: vi.fn(),
      toJSON: () => ({
        endpoint: "https://fcm.googleapis.com/retained",
        keys: { p256dh: "key", auth: "auth" },
      }),
    };
    const subscribe = vi.fn().mockImplementation(async () => {
      current = subscription;
      return subscription;
    });
    vi.stubGlobal("Notification", {
      requestPermission: vi.fn().mockResolvedValue("granted"),
    });
    vi.stubGlobal("navigator", {
      serviceWorker: {
        getRegistration: async () => ({
          active: {},
          pushManager: { getSubscription: async () => current, subscribe },
        }),
      },
    });
    vi.mocked(api.push.subscribe)
      .mockRejectedValueOnce(error)
      .mockResolvedValueOnce(undefined);
    await expect(subscribeDevice("端末")).rejects.toBe(error);
    expect(subscription.unsubscribe).not.toHaveBeenCalled();
    await subscribeDevice("端末");
    expect(subscribe).toHaveBeenCalledOnce();
    expect(api.push.subscribe).toHaveBeenCalledTimes(2);
    expect(vi.mocked(api.push.subscribe).mock.calls[0]).toEqual(
      vi.mocked(api.push.subscribe).mock.calls[1],
    );
  },
);
