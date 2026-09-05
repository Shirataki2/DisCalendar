import { describe, expect, it } from "vitest";
import { CALENDAR_VIEWS } from "./calendar-settings";
import {
  CALENDAR_SHORTCUT_HELP,
  CALENDAR_VIEW_SHORTCUT_KEYS,
  resolveCalendarShortcut,
  type ShortcutKeyInput,
} from "./calendar-shortcuts";

// isShortcutTargetBlocked は DOM が要る (Vitest は node 環境) ので、入力欄で効かないことは E2E で確かめる

function key(key: string, extra: Partial<ShortcutKeyInput> = {}) {
  return {
    key,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
    isComposing: false,
    ...extra,
  };
}

describe("resolveCalendarShortcut", () => {
  it("期間の移動・新規作成・一覧", () => {
    expect(resolveCalendarShortcut(key("t"))).toEqual({ type: "today" });
    expect(resolveCalendarShortcut(key("ArrowLeft"))).toEqual({ type: "prev" });
    expect(resolveCalendarShortcut(key("ArrowRight"))).toEqual({
      type: "next",
    });
    expect(resolveCalendarShortcut(key("n"))).toEqual({ type: "create" });
    expect(resolveCalendarShortcut(key("?"))).toEqual({ type: "help" });
  });

  it("すべてのビューにキーが割り当てられている", () => {
    for (const view of CALENDAR_VIEWS) {
      expect(
        resolveCalendarShortcut(key(CALENDAR_VIEW_SHORTCUT_KEYS[view])),
      ).toEqual({ type: "view", view });
    }
  });

  it("修飾キー付き・変換中・関係ないキーは無視する", () => {
    expect(resolveCalendarShortcut(key("t", { ctrlKey: true }))).toBeNull();
    expect(resolveCalendarShortcut(key("t", { metaKey: true }))).toBeNull();
    expect(resolveCalendarShortcut(key("t", { altKey: true }))).toBeNull();
    expect(resolveCalendarShortcut(key("t", { isComposing: true }))).toBeNull();
    // Shift+t は "T"。大文字には割り当てない
    expect(resolveCalendarShortcut(key("T"))).toBeNull();
    // Shift は "?" 以外では無視する (Shift+← は範囲選択。Playwright は "w" + shiftKey で送る)
    expect(resolveCalendarShortcut(key("w", { shiftKey: true }))).toBeNull();
    expect(
      resolveCalendarShortcut(key("ArrowLeft", { shiftKey: true })),
    ).toBeNull();
    expect(resolveCalendarShortcut(key("?", { shiftKey: true }))).toEqual({
      type: "help",
    });
    // 一部ブラウザが変換中に送るキー
    expect(resolveCalendarShortcut(key("Process"))).toBeNull();
    expect(resolveCalendarShortcut(key("x"))).toBeNull();
  });
});

describe("CALENDAR_SHORTCUT_HELP", () => {
  it("一覧のキーは Esc 以外すべて操作に対応している", () => {
    const keyOf: Record<string, string> = {
      "←": "ArrowLeft",
      "→": "ArrowRight",
    };
    for (const { keys } of CALENDAR_SHORTCUT_HELP) {
      for (const label of keys) {
        if (label === "Esc") continue;
        expect(
          resolveCalendarShortcut(key(keyOf[label] ?? label)),
          label,
        ).not.toBeNull();
      }
    }
  });
});
