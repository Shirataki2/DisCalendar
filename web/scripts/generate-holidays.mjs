// 内閣府の「国民の祝日」CSV から祝日の静的データを生成する (#97)。
// 実行時に外部 API へ依存しないよう、生成結果 (src/lib/japanese-holidays.generated.ts) を
// コミットして同梱する。祝日は毎年 2 月ごろに翌年分が官報告示され CSV に追記されるので、
// 年に一度これを実行してデータを更新する (鮮度は japanese-holidays.test.ts が検査する)。
// 実行: pnpm holidays
import { writeFile } from "node:fs/promises";
import path from "node:path";

const CSV_URL = "https://www8.cao.go.jp/chosei/shukujitsu/syukujitsu.csv";
const OUT_PATH = path.join(
  import.meta.dirname,
  "../src/lib/japanese-holidays.generated.ts",
);

const response = await fetch(CSV_URL);
if (!response.ok) {
  console.error(`CSV の取得に失敗しました: ${response.status} ${CSV_URL}`);
  process.exit(1);
}
// CSV は Shift_JIS (Node の ICU が同梱するデコーダを使う)
const text = new TextDecoder("shift_jis").decode(await response.arrayBuffer());

const holidays = [];
for (const line of text.split(/\r?\n/).slice(1)) {
  if (!line.trim()) continue;
  const [rawDate, name] = line.split(",");
  const match = rawDate.match(/^(\d{4})\/(\d{1,2})\/(\d{1,2})$/);
  if (!match || !name) {
    console.error(`解釈できない行があります: ${line}`);
    process.exit(1);
  }
  const [, year, month, day] = match;
  holidays.push([
    `${year}-${month.padStart(2, "0")}-${day.padStart(2, "0")}`,
    name.trim(),
  ]);
}
if (holidays.length === 0) {
  console.error("CSV から祝日を 1 件も読み取れませんでした");
  process.exit(1);
}

const entries = holidays
  .map(([date, name]) => `  "${date}": "${name}",`)
  .join("\n");
await writeFile(
  OUT_PATH,
  `// このファイルは scripts/generate-holidays.mjs が生成する。手で編集しない (更新は pnpm holidays)。
// 元データ: 内閣府「国民の祝日」CSV (${CSV_URL})

/** 祝日 ("YYYY-MM-DD" → 名称)。振替休日・国民の休日を含む */
export const JAPANESE_HOLIDAYS: Record<string, string> = {
${entries}
};
`,
);
console.log(
  `${holidays.length} 件 (${holidays[0][0]} 〜 ${holidays.at(-1)[0]}) を書き出しました: ${OUT_PATH}`,
);
