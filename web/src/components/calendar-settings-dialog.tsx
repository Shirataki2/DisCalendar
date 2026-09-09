"use client";

import { zodResolver } from "@hookform/resolvers/zod";
import { useState } from "react";
import { Controller, FormProvider, useForm, useWatch } from "react-hook-form";
import { z } from "zod";
import { ColorPicker } from "@/components/form/color-picker";
import { NotificationsField } from "@/components/form/notifications-field";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useCalendarSettings } from "@/hooks/use-calendar-settings";
import {
  CALENDAR_VIEW_LABELS,
  CALENDAR_VIEWS,
  type CalendarInitialView,
  type CalendarSettings,
} from "@/lib/calendar-settings";
import {
  DEFAULT_NOTIFICATIONS,
  eventCreationDefaultsSchema,
  notificationsSchema,
} from "@/lib/event-form";

const INITIAL_VIEW_ITEMS: { value: CalendarInitialView; label: string }[] = [
  ...CALENDAR_VIEWS.map((view) => ({
    value: view,
    label: CALENDAR_VIEW_LABELS[view],
  })),
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
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto">
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
        <CreationDefaultsForm
          settings={settings}
          updateSettings={updateSettings}
        />
      </DialogContent>
    </Dialog>
  );
}

const DURATION_ITEMS = [
  { value: 30, label: "30 分" },
  { value: 60, label: "1 時間" },
  { value: 90, label: "1 時間 30 分" },
  { value: 120, label: "2 時間" },
  { value: 180, label: "3 時間" },
];
const NOTIFICATION_MODE_ITEMS = [
  { value: "server", label: "サーバー設定に従う" },
  { value: "personal", label: "自分で指定" },
];
const creationFormSchema = eventCreationDefaultsSchema
  .omit({ defaultNotifications: true })
  .extend({
    notificationMode: z.enum(["server", "personal"]),
    notifications: notificationsSchema,
  });

// Popup が開くたびにマウントされる。表示設定の即時保存で、編集中の既定値をリセットしない。
function CreationDefaultsForm({
  settings,
  updateSettings,
}: {
  settings: CalendarSettings;
  updateSettings: (patch: Partial<CalendarSettings>) => void;
}) {
  const form = useForm<z.infer<typeof creationFormSchema>>({
    resolver: zodResolver(creationFormSchema),
    defaultValues: {
      defaultColor: settings.defaultColor,
      defaultDurationMinutes: settings.defaultDurationMinutes,
      notificationMode:
        settings.defaultNotifications === null ? "server" : "personal",
      notifications:
        settings.defaultNotifications ??
        DEFAULT_NOTIFICATIONS.map((notification) => ({ ...notification })),
    },
  });
  const mode = useWatch({ control: form.control, name: "notificationMode" });
  const [saved, setSaved] = useState(false);
  return (
    <FormProvider {...form}>
      <form
        className="flex flex-col gap-4 border-t pt-4"
        onSubmit={form.handleSubmit((values) => {
          updateSettings({
            defaultColor: values.defaultColor,
            defaultDurationMinutes: values.defaultDurationMinutes,
            defaultNotifications:
              values.notificationMode === "server"
                ? null
                : values.notifications,
          });
          form.reset(values);
          setSaved(true);
        })}
      >
        <h3 className="text-sm font-medium">新規作成の既定値</h3>
        <p className="text-sm text-muted-foreground">
          保存後、新しく予定を作るときに使われます。編集・複製では元の予定の値を使います。
        </p>
        <Field>
          <FieldLabel htmlFor="calendar-settings-color">既定の色</FieldLabel>
          <Controller
            control={form.control}
            name="defaultColor"
            render={({ field }) => (
              <ColorPicker
                id="calendar-settings-color"
                value={field.value}
                onChange={(value) => {
                  field.onChange(value);
                }}
              />
            )}
          />
          <FieldError errors={[form.formState.errors.defaultColor]} />
        </Field>
        <Field>
          <FieldLabel htmlFor="calendar-settings-duration">
            クリックで作るときの長さ
          </FieldLabel>
          <Controller
            control={form.control}
            name="defaultDurationMinutes"
            render={({ field }) => (
              <Select
                value={field.value}
                items={DURATION_ITEMS}
                onValueChange={(value) => {
                  if (value !== null) {
                    field.onChange(value);
                  }
                }}
              >
                <SelectTrigger
                  id="calendar-settings-duration"
                  className="w-full"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {DURATION_ITEMS.map((item) => (
                    <SelectItem key={item.value} value={item.value}>
                      {item.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          />
          <p className="text-sm text-muted-foreground">
            時間帯を範囲選択したときは、その範囲を優先します。終日の日付クリックでは、終日を外したときの長さに使います。
          </p>
        </Field>
        <Field>
          <FieldLabel htmlFor="calendar-settings-notification-mode">
            事前通知の既定値
          </FieldLabel>
          <Controller
            control={form.control}
            name="notificationMode"
            render={({ field }) => (
              <Select
                value={field.value}
                items={NOTIFICATION_MODE_ITEMS}
                onValueChange={(value) => {
                  if (!value) return;
                  field.onChange(value);
                  if (value === "server")
                    form.setValue(
                      "notifications",
                      DEFAULT_NOTIFICATIONS.map((notification) => ({
                        ...notification,
                      })),
                      { shouldValidate: true },
                    );
                }}
              >
                <SelectTrigger
                  id="calendar-settings-notification-mode"
                  className="w-full"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {NOTIFICATION_MODE_ITEMS.map((item) => (
                    <SelectItem key={item.value} value={item.value}>
                      {item.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          />
        </Field>
        {mode === "personal" && (
          <NotificationsField label="既定の事前通知">
            <p className="text-sm text-muted-foreground">
              すべて削除すると事前通知なしになります。サーバーの既定値より優先されます。
            </p>
          </NotificationsField>
        )}
        <Button type="submit">既定値を保存</Button>
        {saved && !form.formState.isDirty && (
          <p role="status" className="text-sm text-muted-foreground">
            既定値を保存しました。
          </p>
        )}
      </form>
    </FormProvider>
  );
}
