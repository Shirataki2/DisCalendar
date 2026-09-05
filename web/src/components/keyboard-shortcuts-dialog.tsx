"use client";

import { createContext, Fragment, useContext } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { CALENDAR_SHORTCUT_HELP } from "@/lib/calendar-shortcuts";

/**
 * ショートカット一覧のダイアログを開く関数。ダイアログ本体は DashboardShell が持ち
 * (ドロワー (Sheet) の中に置くと閉じたときにアンマウントされる #96 の教訓)、
 * アカウントメニューとカレンダーの "?" キーはこの context 経由で開く。
 * ダッシュボードの外 (管理コンソールのカレンダー) では null で、"?" は何もしない
 */
export const OpenKeyboardShortcutsContext = createContext<(() => void) | null>(
  null,
);

export function useOpenKeyboardShortcuts(): (() => void) | null {
  return useContext(OpenKeyboardShortcutsContext);
}

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** カレンダーのキーボードショートカット一覧 (#160)。内容は lib/calendar-shortcuts.ts の表から作る */
export function KeyboardShortcutsDialog({ open, onOpenChange }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>キーボードショートカット</DialogTitle>
          <DialogDescription>
            カレンダー画面で使えるキー操作です。文字を入力している間やダイアログを開いている間は使えません。
          </DialogDescription>
        </DialogHeader>
        <dl className="grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-2">
          {CALENDAR_SHORTCUT_HELP.map(({ keys, description }) => (
            <Fragment key={description}>
              <dt className="flex justify-end gap-1">
                {keys.map((key) => (
                  <kbd
                    key={key}
                    className="inline-flex h-6 min-w-6 items-center justify-center rounded-md border border-border bg-muted px-1.5 font-mono text-xs"
                  >
                    {key}
                  </kbd>
                ))}
              </dt>
              <dd>{description}</dd>
            </Fragment>
          ))}
        </dl>
      </DialogContent>
    </Dialog>
  );
}
