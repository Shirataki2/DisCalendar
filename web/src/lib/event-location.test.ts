import { describe, expect, it } from "vitest";
import { isValidLocation, locationUrl } from "./event-location";

describe("予定の場所", () => {
  it("場所名のコロンを許可し、URL は安全に正規化する", () => {
    expect(isValidLocation("Room: A")).toBe(true);
    expect(isValidLocation("javascript:alert(1)")).toBe(false);
    expect(locationUrl("https://example.com/会議 室\r\nSUMMARY:injected")).toBe(
      "https://example.com/%E4%BC%9A%E8%AD%B0%20%E5%AE%A4SUMMARY:injected",
    );
  });
});
