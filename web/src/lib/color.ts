/**
 * `#RRGGBB` の背景に載せる文字色 (白か黒)。
 * 予定の色は利用者が自由に選べるので、黄色などの明るい色でも読めるようにする
 */
export function readableTextColor(hex: string): "#000000" | "#ffffff" {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!match) return "#ffffff";
  const [r, g, b] = match.slice(1, 4).map((channel) => {
    const value = Number.parseInt(channel, 16) / 255;
    return value <= 0.03928 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return luminance > 0.5 ? "#000000" : "#ffffff";
}
