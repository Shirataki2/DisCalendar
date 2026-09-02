// 横断カレンダー (#98) でサーバーごとに割り当てる色。
// 予定自体の色は使わず、凡例と同じ色で「どのサーバーの予定か」を見せる

/**
 * サーバーに割り当てる色の並び。ライト / ダークどちらの地でも予定として目立ち、
 * `readableTextColor` の白か黒の文字が読める彩度・明度にしてある。隣り合う色は色相を離す
 */
export const GUILD_COLOR_PALETTE: readonly string[] = [
  "#1E88E5", // 青
  "#E53935", // 赤
  "#43A047", // 緑
  "#FB8C00", // 橙
  "#8E24AA", // 紫
  "#00ACC1", // 水色
  "#F4511E", // 朱
  "#3949AB", // 藍
  "#7CB342", // 黄緑
  "#D81B60", // 桃
];

/** index 番目の色。パレットを超えたら先頭から巡回する (11 サーバー以上は色が重なるが凡例で見分けられる) */
export function guildColorAt(index: number): string {
  return GUILD_COLOR_PALETTE[
    ((index % GUILD_COLOR_PALETTE.length) + GUILD_COLOR_PALETTE.length) %
      GUILD_COLOR_PALETTE.length
  ];
}

/** サーバー一覧の並び順で色を割り当てる (guild_id → 色) */
export function assignGuildColors(
  guildIds: readonly string[],
): Map<string, string> {
  return new Map(guildIds.map((id, index) => [id, guildColorAt(index)]));
}
