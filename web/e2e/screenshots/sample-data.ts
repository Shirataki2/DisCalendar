// LP / 使い方のスクリーンショットに写すサンプルの予定。
// 撮影のたびに日付が古びないよう、ブラウザから見た「今月」を基準に組み立てる (絶対日付を書かない)。
// 内容は架空のサーバー (fixtures.ts の撮影用の名前) の 1 か月ぶんという想定

/** POST /events/{guild_id} の body。日時はタイムゾーンなしの JST 文字列 (api の保存形式と同じ) */
export interface SampleEvent {
  name: string;
  description: string | null;
  notifications: { num: number; unit: string }[];
  color: string;
  is_all_day: boolean;
  start_at: string;
  end_at: string;
  /** Discord のイベントとしても作成する (#94。編集ダイアログの画像にチェック済みの状態を写す) */
  discord_scheduled_event?: boolean;
}

/** 編集ダイアログ (lp/dialog.png) を開く予定 */
export const EDIT_TARGET = "ゲーム大会 (カスタムマッチ)";
/** ポップオーバー (docs/popover.png) を開く予定 */
export const POPOVER_TARGET = "新メンバー歓迎会";

/** 作成ダイアログ (docs/create-dialog.png) に入力する内容 (保存はしない) */
export const CREATE_SAMPLE = {
  name: "もくもく会",
  startTime: "19:30",
  endTime: "21:00",
  description: "作業したい人はボイスチャンネルに集合。途中参加・退出は自由です",
} as const;

/**
 * `today` の月に収まるサンプルの予定。
 * 日付は月の長さに合わせて切り捨てるので、28 日までの月でも欠けた予定が残らない
 */
export function sampleEvents(today: Date): SampleEvent[] {
  const year = today.getFullYear();
  const month = today.getMonth();
  const lastDay = new Date(year, month + 1, 0).getDate();

  const at = (day: number, time: string) =>
    `${year}-${pad(month + 1)}-${pad(day)}T${time}:00`;
  const timed = (
    day: number,
    name: string,
    start: string,
    end: string,
    color: string,
    extra: Partial<SampleEvent> = {},
  ): SampleEvent => ({
    name,
    description: null,
    notifications: [],
    color,
    is_all_day: false,
    start_at: at(day, start),
    end_at: at(day, end),
    ...extra,
  });
  /** 終日予定の end_at は「終了日 (含む) の 0:00」(lib/calendar-events.ts の toApiRange と同じ) */
  const allDay = (
    from: number,
    to: number,
    name: string,
    color: string,
  ): SampleEvent => ({
    name,
    description: null,
    notifications: [],
    color,
    is_all_day: true,
    start_at: at(from, "00:00"),
    end_at: at(to, "00:00"),
  });

  // 編集ダイアログ (lp/dialog.png) に Discord 連携のチェックが入った状態を写すため、
  // この予定だけ撮影日の 2 日後に置く (開始が過去だと連携できないので、未来の日付が要る)。
  // 月末の撮影で翌々日が翌月になるときだけ月内に収め、連携は諦める (作成が 400 で落ちないように)
  const editDay = Math.min(today.getDate() + 2, lastDay);
  const editIsFuture = editDay > today.getDate();

  const events: SampleEvent[] = [
    // 毎週の定例。曜日で並ぶので、どの月に撮っても縦にきれいに揃う
    ...mondays(year, month).map((day) =>
      timed(day, "定例ミーティング", "21:00", "22:00", "#5865f2"),
    ),
    // 深夜の予定 (時刻が "2:00" と出ることの確認も兼ねる)
    timed(5, "メンテナンス", "02:00", "04:00", "#95a5a6"),
    timed(editDay, EDIT_TARGET, "20:00", "23:30", "#e91e63", {
      description: "参加者はボイスチャンネルに集合。賞品あり",
      notifications: [
        { num: 30, unit: "minutes" },
        { num: 1, unit: "days" },
      ],
      discord_scheduled_event: editIsFuture,
    }),
    allDay(13, 15, "合宿", "#009688"),
    timed(20, "配信: 新作レビュー", "20:00", "22:00", "#9b59b6"),
    // ポップオーバーの画像には説明も写す (docs/edit.mdx の alt に合わせる)
    timed(21, POPOVER_TARGET, "19:30", "21:00", "#ff9800", {
      description: "はじめての人も歓迎です。まずは自己紹介から",
      notifications: [{ num: 1, unit: "hours" }],
    }),
    timed(21, "企画会議", "21:30", "23:00", "#00bcd4"),
    allDay(26, 26, "サーバー 3 周年", "#4caf50"),
    timed(29, "月例オフ会", "18:00", "21:00", "#607d8b"),
  ];

  return events.filter((event) => dayOf(event.end_at) <= lastDay);
}

/** その月の月曜日 (定例ミーティングを置く日) */
function mondays(year: number, month: number): number[] {
  const lastDay = new Date(year, month + 1, 0).getDate();
  const days: number[] = [];
  for (let day = 1; day <= lastDay; day++) {
    if (new Date(year, month, day).getDay() === 1) days.push(day);
  }
  return days;
}

function dayOf(apiDateTime: string): number {
  return Number(apiDateTime.slice(8, 10));
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}
