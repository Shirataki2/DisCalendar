// 管理コンソール (#33〜) の表示用フォーマット。
// api が返す日時は UTC の ISO 8601 (監査ログ・セッション) と JST の naive 文字列 (予定) の 2 種類がある。
// 運用者が見るのは JST なので、どちらも JST に揃えて出す。

/** ブラウザのロケールに関係なく JST で揃える (運用者が見る時刻は JST) */
const JST = "Asia/Tokyo";

/** UTC の ISO 8601 (`2026-08-23T01:00:00Z`) を JST の日時にする */
export function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleString("ja-JP", {
    timeZone: JST,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** 年を省いた短い形式 (履歴など、直近しか見ない一覧向け) */
export function formatShortDateTime(iso: string): string {
  return new Date(iso).toLocaleString("ja-JP", {
    timeZone: JST,
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/**
 * タイムゾーンなしの JST 文字列 (`2026-08-23T10:00:00`。予定や `stats.day_start`) を読みやすくする。
 * `new Date()` に渡すとブラウザのタイムゾーンで解釈されてずれるので、文字列のまま整形する
 */
export function formatNaiveJst(value: string): string {
  const match = value.match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/);
  if (!match) return value;
  const [, year, month, day, hour, minute] = match;
  return `${year}/${month}/${day} ${hour}:${minute}`;
}

/** 秒数を「3日 4時間 5分」のような文字列にする (稼働時間の表示) */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "-";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (days > 0) return `${days}日 ${hours}時間 ${minutes}分`;
  if (hours > 0) return `${hours}時間 ${minutes}分`;
  if (minutes > 0) return `${minutes}分`;
  return `${Math.floor(seconds)}秒`;
}
