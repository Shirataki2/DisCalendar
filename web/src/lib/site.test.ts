import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { OG_IMAGE_ALT, pageOpenGraph } from "./site";

describe("pageOpenGraph", () => {
  it("サイト共通の項目と画像を付ける (子の openGraph が親の分を置き換えるため)", () => {
    expect(
      pageOpenGraph({ title: "使い方", description: "説明", url: "/docs/x" }),
    ).toEqual({
      type: "website",
      locale: "ja_JP",
      siteName: "DisCalendar",
      title: "使い方",
      description: "説明",
      url: "/docs/x",
      images: [
        {
          url: "/opengraph-image.png",
          width: 1200,
          height: 630,
          type: "image/png",
          alt: OG_IMAGE_ALT,
        },
      ],
    });
  });

  it("image: false なら画像を付けない (同じセグメントの opengraph-image を使わせる)", () => {
    const og = pageOpenGraph({
      title: "予定",
      description: "説明",
      url: "/share/t",
      image: false,
    });
    expect(og).not.toHaveProperty("images");
    expect(og).toMatchObject({ siteName: "DisCalendar", locale: "ja_JP" });
  });

  it("画像の代替テキストは app/opengraph-image.alt.txt と同じ", () => {
    const alt = readFileSync(
      new URL("../app/opengraph-image.alt.txt", import.meta.url),
      "utf8",
    );
    expect(OG_IMAGE_ALT).toBe(alt.trim());
  });
});
