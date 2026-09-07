import { afterEach, expect, it, vi } from "vitest";
import { ApiError, api } from "@/lib/api";
import { unsubscribeCurrentDevice } from "./push";

vi.mock("@/lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/api")>()),
  api: { push: { removeCurrent: vi.fn() } },
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
