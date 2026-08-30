"use client";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useCalendarSettings } from "@/hooks/use-calendar-settings";
import type { CalendarInitialView } from "@/lib/calendar-settings";

const INITIAL_VIEW_ITEMS: { value: CalendarInitialView; label: string }[] = [
  { value: "dayGridMonth", label: "月" },
  { value: "timeGridWeek", label: "週" },
  { value: "timeGridFourDay", label: "4日" },
  { value: "timeGridDay", label: "日" },
  { value: "listMonth", label: "リスト" },
  { value: "last", label: "前回開いていたビュー" },
];

// Select の値は文字列で持ち、保存時に firstDay の数値へ起こす
const FIRST_DAY_ITEMS: { value: "0" | "1"; label: string }[] = [
  { value: "0", label: "日曜日" },
  { value: "1", label: "月曜日" },
];

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/**
 * カレンダーの表示設定ダイアログ (#96)。テーマ (#58) と同じ端末ごとの個人設定で、
 * 選択はその場で localStorage に保存する。アカウントメニューとドロワーの両方から開く
 */
export function CalendarSettingsDialog({ open, onOpenChange }: Props) {
  const { settings, updateSettings } = useCalendarSettings();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>カレンダーの表示設定</DialogTitle>
          <DialogDescription>
            設定はこのブラウザに記憶されます。週の開始曜日はすぐに反映され、最初に表示するビューは次にカレンダーを開いたときから使われます。
          </DialogDescription>
        </DialogHeader>
        <Field>
          <FieldLabel htmlFor="calendar-settings-initial-view">
            最初に表示するビュー
          </FieldLabel>
          <Select
            value={settings.initialView}
            onValueChange={(value) => {
              if (value) updateSettings({ initialView: value });
            }}
            items={INITIAL_VIEW_ITEMS}
          >
            <SelectTrigger
              id="calendar-settings-initial-view"
              className="w-full"
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {INITIAL_VIEW_ITEMS.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
        <Field>
          <FieldLabel htmlFor="calendar-settings-first-day">
            週の開始曜日
          </FieldLabel>
          <Select
            value={String(settings.firstDay)}
            onValueChange={(value) => {
              if (value) updateSettings({ firstDay: value === "1" ? 1 : 0 });
            }}
            items={FIRST_DAY_ITEMS}
          >
            <SelectTrigger id="calendar-settings-first-day" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {FIRST_DAY_ITEMS.map((item) => (
                <SelectItem key={item.value} value={item.value}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </Field>
      </DialogContent>
    </Dialog>
  );
}
