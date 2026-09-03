import { describe, expect, it } from "vitest";
import { buildFeedUrl, feedPath } from "./feed-url";

const TOKEN =
  "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

describe("buildFeedUrl", () => {
  it("オリジンと /feeds/<token>.ics を繋ぐ", () => {
    expect(feedPath(TOKEN)).toBe(`/feeds/${TOKEN}.ics`);
    expect(buildFeedUrl("https://discalendar.app", TOKEN)).toBe(
      `https://discalendar.app/feeds/${TOKEN}.ics`,
    );
    expect(buildFeedUrl("http://localhost:3000", TOKEN)).toBe(
      `http://localhost:3000/feeds/${TOKEN}.ics`,
    );
  });

  it("オリジン末尾のスラッシュを重ねない", () => {
    expect(buildFeedUrl("https://discalendar.app/", TOKEN)).toBe(
      `https://discalendar.app/feeds/${TOKEN}.ics`,
    );
  });
});
