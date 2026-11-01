import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("server-only", () => ({}));

const { recordInviteClick } = await import("./invite-clicks");

describe("recordInviteClick", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("導入元だけを api に送る", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await recordInviteClick("lp-hero");

    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(String(url)).toMatch(/\/invite-clicks$/);
    expect(init.method).toBe("POST");
    expect(JSON.parse(init.body)).toEqual({ source: "lp-hero" });
    // cookie などの利用者の情報は付けない
    expect(init.headers).toEqual({ "content-type": "application/json" });
  });

  it("from が無ければ null、長い値は切り詰めて送る", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await recordInviteClick(null);
    await recordInviteClick("x".repeat(500));

    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({
      source: null,
    });
    expect(JSON.parse(fetchMock.mock.calls[1][1].body).source).toHaveLength(64);
  });

  it("api に届かなくても例外を投げない", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new Error("down")));
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    await expect(recordInviteClick("docs-start")).resolves.toBeUndefined();
    expect(warn).toHaveBeenCalled();
  });
});
