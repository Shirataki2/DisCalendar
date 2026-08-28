import { describe, expect, it } from "vitest";
import {
  ApiError,
  buildInit,
  describeApiError,
  handleResponse,
} from "@/lib/api/client";

function jsonResponse(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("buildInit", () => {
  it("body があれば JSON にして content-type を付ける", () => {
    const init = buildInit({ method: "POST", body: { restricted: true } });
    expect(init).toEqual({
      method: "POST",
      headers: { "content-type": "application/json" },
      body: '{"restricted":true}',
      signal: undefined,
    });
  });

  it("body がなければ GET で headers も付けない", () => {
    const controller = new AbortController();
    expect(buildInit({ signal: controller.signal })).toEqual({
      method: "GET",
      headers: undefined,
      body: undefined,
      signal: controller.signal,
    });
  });
});

describe("handleResponse", () => {
  it("2xx は JSON を返し、204 は undefined", async () => {
    await expect(handleResponse(jsonResponse(200, { ok: 1 }))).resolves.toEqual(
      { ok: 1 },
    );
    await expect(
      handleResponse(new Response(null, { status: 204 })),
    ).resolves.toBeUndefined();
  });

  it("api のエラー形式 ({ error, message }) を ApiError にする", async () => {
    const error = await handleResponse(
      jsonResponse(403, { error: "forbidden", message: "no permission" }),
    ).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({
      name: "ApiError",
      status: 403,
      kind: "forbidden",
      message: "no permission",
    });
  });

  it("JSON でないレスポンス (プロキシ先に届かないなど) は kind が unknown", async () => {
    const error = await handleResponse(
      new Response("Bad Gateway", { status: 502 }),
    ).catch((e: unknown) => e);
    expect(error).toMatchObject({
      status: 502,
      kind: "unknown",
      message: "API request failed with status 502",
    });
  });
});

describe("describeApiError", () => {
  const cases: [ApiError["kind"], number, string][] = [
    [
      "unauthorized",
      401,
      "ログインの有効期限が切れました。再度ログインしてください",
    ],
    ["forbidden", 403, "この操作を行う権限がありません"],
    [
      "not_found",
      404,
      "対象が見つかりません (他のユーザーが削除した可能性があります)",
    ],
    [
      "conflict",
      409,
      "他の更新と同時に行われたため保存できませんでした。もう一度お試しください",
    ],
    [
      "rate_limited",
      429,
      "Discord API の制限中です。しばらく待ってから再度お試しください",
    ],
    [
      "discord_error",
      502,
      "Discord との通信に失敗しました。時間をおいて再度お試しください",
    ],
  ];
  it.each(cases)("%s は固定の日本語メッセージ", (kind, status, expected) => {
    expect(describeApiError(new ApiError(status, kind, "raw"))).toBe(expected);
  });

  it("bad_request / unavailable は API のメッセージを添える", () => {
    expect(
      describeApiError(new ApiError(400, "bad_request", "name is too long")),
    ).toBe("入力内容が正しくありません (name is too long)");
    expect(
      describeApiError(new ApiError(503, "unavailable", "role missing")),
    ).toBe("この機能は現在使えません (role missing)");
  });

  it("それ以外の ApiError はステータスを出す", () => {
    expect(describeApiError(new ApiError(500, "internal_error", "x"))).toBe(
      "サーバーでエラーが発生しました (500)",
    );
    expect(describeApiError(new ApiError(502, "unknown", "x"))).toBe(
      "サーバーでエラーが発生しました (502)",
    );
  });

  it("ApiError 以外は中断かネットワークエラーとして扱う", () => {
    const abort = new Error("aborted");
    abort.name = "AbortError";
    expect(describeApiError(abort)).toBe("通信が中断されました");
    expect(describeApiError(new TypeError("Failed to fetch"))).toBe(
      "通信に失敗しました。ネットワークを確認してください",
    );
    expect(describeApiError("oops")).toBe(
      "通信に失敗しました。ネットワークを確認してください",
    );
  });
});
