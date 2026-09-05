import {
  CALENDAR_VIEW_LABELS,
  CALENDAR_VIEWS,
  type CalendarView,
} from "@/lib/calendar-settings";

/**
 * カレンダーのキーボードショートカット (#160)。割り当ては Google カレンダーに寄せる。
 * キーとビューの対応はここに寄せ、keydown を処理するフック (hooks/use-calendar-shortcuts) と
 * 一覧ダイアログ (components/keyboard-shortcuts-dialog) の両方がこの表を見る。
 * 年表示 (#157) が入ったら "y" を足す
 */

/** ビュー切替のキー */
export const CALENDAR_VIEW_SHORTCUT_KEYS: Record<CalendarView, string> = {
  dayGridMonth: "m",
  timeGridWeek: "w",
  timeGridFourDay: "4",
  timeGridDay: "d",
  listMonth: "l",
};

export type CalendarShortcutAction =
  | { type: "today" }
  | { type: "prev" }
  | { type: "next" }
  | { type: "create" }
  | { type: "view"; view: CalendarView }
  | { type: "help" };

/** keydown のうち判定に使う項目 (テストで KeyboardEvent を組み立てずに済むよう最小限にする) */
export interface ShortcutKeyInput {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  isComposing: boolean;
}

/**
 * 押されたキーに対応する操作。該当しなければ null。
 * 修飾キー (Ctrl / Cmd / Alt) 付きはブラウザや OS のショートカットなので奪わない。
 * Shift は "?" を打つのに要るのでそのときだけ許す (Shift+← は範囲選択などに使われる。
 * 文字キーは実ブラウザなら "W" になって一致しないが、Playwright は "w" + shiftKey で送る)。
 * 日本語入力の変換中 (isComposing) は無視する
 */
export function resolveCalendarShortcut(
  input: ShortcutKeyInput,
): CalendarShortcutAction | null {
  if (input.ctrlKey || input.metaKey || input.altKey || input.isComposing) {
    return null;
  }
  if (input.shiftKey && input.key !== "?") return null;
  switch (input.key) {
    case "t":
      return { type: "today" };
    case "ArrowLeft":
      return { type: "prev" };
    case "ArrowRight":
      return { type: "next" };
    case "n":
      return { type: "create" };
    case "?":
      return { type: "help" };
  }
  const view = CALENDAR_VIEWS.find(
    (candidate) => CALENDAR_VIEW_SHORTCUT_KEYS[candidate] === input.key,
  );
  return view ? { type: "view", view } : null;
}

/**
 * フォーカスのある要素から見て、キー入力を受け付けない場面か。
 * - 入力欄・テキストエリア・Select・contenteditable: 文字を打っている最中に画面を切り替えない
 * - ダイアログ・ドロワー (Sheet)・ポップオーバー・メニューの中: 開いているものの操作を優先する
 *   (モーダルはフォーカスを閉じ込めるので、開いている間の keydown はここで弾ける)
 */
export function isShortcutTargetBlocked(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (target instanceof HTMLElement && target.isContentEditable) return true;
  return (
    target.closest(
      'input, textarea, select, [role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]',
    ) !== null
  );
}

/** 一覧ダイアログに載せる説明。表示順もこのとおり (使い方の表も同じ並びにする) */
export interface CalendarShortcutHelp {
  /** 表示するキー。複数あれば並べて出す ("←" "→" など) */
  keys: readonly string[];
  description: string;
}

export const CALENDAR_SHORTCUT_HELP: readonly CalendarShortcutHelp[] = [
  { keys: ["t"], description: "今日に戻る" },
  { keys: ["←", "→"], description: "前 / 次の期間へ移動" },
  {
    keys: ["n"],
    description: "予定を新規作成 (予定を編集できるサーバーのカレンダーのみ)",
  },
  ...CALENDAR_VIEWS.map((view) => ({
    keys: [CALENDAR_VIEW_SHORTCUT_KEYS[view]],
    description: `${CALENDAR_VIEW_LABELS[view]}表示に切り替え`,
  })),
  { keys: ["?"], description: "この一覧を表示" },
  { keys: ["Esc"], description: "開いている予定の詳細やダイアログを閉じる" },
];
