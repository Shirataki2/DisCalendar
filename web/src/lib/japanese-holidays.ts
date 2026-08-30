// 日本の祝日の表示 (#97)。データは内閣府 CSV から生成した静的データを同梱し、
// 実行時の外部 API には依存しない (更新は pnpm holidays)
import { JAPANESE_HOLIDAYS } from "./japanese-holidays.generated";

/**
 * 祝日名を返す (祝日でなければ undefined)。
 * FullCalendar の描画フック (dayCellTopContent / dayCellTopInnerClass など) に渡る date は、
 * 内部の DateMarker (UTC 基準) と違い実タイムゾーンの Date に変換済み (dateEnv.toDate) なので、
 * 表示上の日付はローカルのゲッターで取り出す
 */
export function holidayNameOf(date: Date): string | undefined {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return JAPANESE_HOLIDAYS[`${date.getFullYear()}-${month}-${day}`];
}

/**
 * 曜日だけの配色クラス (globals.css で定義)。日本のカレンダーの慣例どおり日曜は赤、土曜は青。
 * 月ビューの曜日ヘッダのような特定の日付を指さない場所に使う
 */
export function dowColorClass(dow: number): string | undefined {
  if (dow === 0) return "cal-day-red";
  if (dow === 6) return "cal-day-blue";
  return undefined;
}

/** 日付セル・日付ヘッダの配色クラス。曜日の色に加えて祝日を赤にする */
export function dayColorClass(info: {
  date: Date;
  dow: number;
}): string | undefined {
  if (holidayNameOf(info.date)) return "cal-day-red";
  return dowColorClass(info.dow);
}
