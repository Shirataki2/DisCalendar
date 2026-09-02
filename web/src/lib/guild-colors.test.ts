import { describe, expect, it } from "vitest";
import { readableTextColor } from "./color";
import {
  assignGuildColors,
  GUILD_COLOR_PALETTE,
  guildColorAt,
} from "./guild-colors";

describe("guildColorAt", () => {
  it("並び順どおりに割り当て、パレットを超えたら巡回する", () => {
    expect(guildColorAt(0)).toBe(GUILD_COLOR_PALETTE[0]);
    expect(guildColorAt(GUILD_COLOR_PALETTE.length - 1)).toBe(
      GUILD_COLOR_PALETTE[GUILD_COLOR_PALETTE.length - 1],
    );
    expect(guildColorAt(GUILD_COLOR_PALETTE.length)).toBe(
      GUILD_COLOR_PALETTE[0],
    );
    expect(guildColorAt(GUILD_COLOR_PALETTE.length + 1)).toBe(
      GUILD_COLOR_PALETTE[1],
    );
  });

  it("パレットは #RRGGBB で、どの色にも白か黒の文字を載せられる", () => {
    for (const color of GUILD_COLOR_PALETTE) {
      expect(color).toMatch(/^#[0-9A-F]{6}$/);
      expect(["#000000", "#ffffff"]).toContain(readableTextColor(color));
    }
    // 見分けるための色なので重複しない
    expect(new Set(GUILD_COLOR_PALETTE).size).toBe(GUILD_COLOR_PALETTE.length);
  });
});

describe("assignGuildColors", () => {
  it("サーバー一覧の並び順で色を割り当てる", () => {
    const colors = assignGuildColors([
      "200000000000000002",
      "200000000000000001",
    ]);
    expect(colors.get("200000000000000002")).toBe(GUILD_COLOR_PALETTE[0]);
    expect(colors.get("200000000000000001")).toBe(GUILD_COLOR_PALETTE[1]);
    expect(colors.get("999")).toBeUndefined();
  });
});
