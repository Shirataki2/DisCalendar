import { describe, expect, it } from "vitest";
import { externalLinkUrl } from "./links";

const CURRENT = "https://123.discordsays.com/activity?frame_id=f";
const SITE = "https://discalendar.app";

describe("externalLinkUrl", () => {
  it("別オリジンのリンクはそのまま外で開く", () => {
    expect(
      externalLinkUrl("https://example.com/a", "_blank", CURRENT, SITE),
    ).toBe("https://example.com/a");
    expect(externalLinkUrl("https://example.com/a", null, CURRENT, SITE)).toBe(
      "https://example.com/a",
    );
  });

  it("同じオリジンの Activity 以外の画面は公開 URL に直して外で開く", () => {
    expect(externalLinkUrl("/docs/invite", "_blank", CURRENT, SITE)).toBe(
      "https://discalendar.app/docs/invite",
    );
    expect(
      externalLinkUrl("/dashboard/1/polls?x=1#y", null, CURRENT, SITE),
    ).toBe("https://discalendar.app/dashboard/1/polls?x=1#y");
  });

  it("Activity の中へのリンクやページ内リンクは開かない", () => {
    expect(externalLinkUrl("/activity", null, CURRENT, SITE)).toBeNull();
    expect(externalLinkUrl("#top", null, CURRENT, SITE)).toBeNull();
    expect(externalLinkUrl("", null, CURRENT, SITE)).toBeNull();
  });

  it("http(s) 以外は扱わない", () => {
    expect(
      externalLinkUrl("mailto:a@example.com", null, CURRENT, SITE),
    ).toBeNull();
    expect(
      externalLinkUrl("javascript:void 0", null, CURRENT, SITE),
    ).toBeNull();
  });
});
