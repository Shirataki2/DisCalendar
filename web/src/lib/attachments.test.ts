import { afterEach, expect, test, vi } from "vitest";
import { ApiError, api } from "@/lib/api";
import {
  attachmentValidation,
  type PendingAttachment,
  uploadAttachment,
} from "./attachments";
import { invalidateEvents } from "./query/events";
import { queryKeys } from "./query/keys";

vi.mock("./query/admin-cache", () => ({
  revalidateAdminPagesQuietly: vi.fn(),
}));

vi.mock("@/lib/api", async (original) => {
  const actual = await original<typeof import("@/lib/api")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      attachments: { reserve: vi.fn(), complete: vi.fn(), remove: vi.fn() },
    },
  };
});

test("予定削除と管理画面の一括削除は添付一覧・使用量のキャッシュも更新する", async () => {
  const client = new QueryClient();
  const limits = queryKeys.attachments.limits("111");
  const files = queryKeys.attachments.event("111", 1);
  const other = queryKeys.attachments.limits("222");
  for (const key of [limits, files, other]) client.setQueryData(key, []);
  await invalidateEvents(
    client,
    { client: api.events, keys: queryKeys.events },
    "111",
    true,
  );
  expect(client.getQueryState(limits)?.isInvalidated).toBe(true);
  expect(client.getQueryState(files)?.isInvalidated).toBe(true);
  expect(client.getQueryState(other)?.isInvalidated).toBe(false);
  client.clear();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});

const item = (): PendingAttachment => ({
  key: "file",
  file: new File(["%PDF-1.7"], "案内.pdf"),
  status: "failed",
  eventId: 1,
  reservation: {
    id: "reserved",
    upload: {
      url: "https://example.test/upload",
      headers: {},
      expires_at: new Date(Date.now() + 300000).toISOString(),
    },
  },
});

test("完了応答を失った再試行では予定も予約も再作成せず、確定結果を取得する", async () => {
  const fetch = vi.fn();
  vi.stubGlobal("fetch", fetch);
  vi.mocked(api.attachments.complete).mockResolvedValue({
    id: "reserved",
    filename: "案内.pdf",
    size: 8,
    content_type: "application/pdf",
    created_at: "",
  });
  const result = await uploadAttachment(
    "111",
    1,
    item(),
    new AbortController().signal,
    vi.fn(),
  );
  expect(result.id).toBe("reserved");
  expect(fetch).not.toHaveBeenCalled();
  expect(api.attachments.reserve).not.toHaveBeenCalled();
});

test("送信失敗後は同じ予約へ再送し、既存オブジェクトの412でも検証を省かない", async () => {
  vi.mocked(api.attachments.complete)
    .mockRejectedValueOnce(new ApiError(503, "unavailable", "retry"))
    .mockResolvedValueOnce({
      id: "reserved",
      filename: "案内.pdf",
      size: 8,
      content_type: "application/pdf",
      created_at: "",
    });
  const fetch = vi.fn().mockResolvedValue({ ok: false, status: 412 });
  vi.stubGlobal("fetch", fetch);
  await uploadAttachment(
    "111",
    1,
    item(),
    new AbortController().signal,
    vi.fn(),
  );
  expect(fetch).toHaveBeenCalledOnce();
  expect(api.attachments.complete).toHaveBeenCalledTimes(2);
  expect(api.attachments.reserve).not.toHaveBeenCalled();
});

test("形式・サイズ上限を選択時に検証する", () => {
  expect(attachmentValidation(new File(["x"], "案内.PDF"))).toBeNull();
  expect(attachmentValidation(new File([], "空.pdf"))).toBeTruthy();
  expect(attachmentValidation(new File(["x"], "案内.svg"))).toBeTruthy();
  expect(
    attachmentValidation(new File([new Uint8Array(10485760)], "上限.pdf")),
  ).toBeNull();
  expect(
    attachmentValidation(new File([new Uint8Array(10485761)], "超過.pdf")),
  ).toBeTruthy();
});

import { QueryClient } from "@tanstack/react-query";
