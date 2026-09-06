"use client";

import { PlusIcon, XIcon } from "lucide-react";
import { Controller, useFieldArray, useFormContext } from "react-hook-form";
import { Button } from "@/components/ui/button";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Notification } from "@/lib/api/types";
import {
  NOTIFICATION_NUM_MAX,
  NOTIFICATION_NUM_MIN,
  NOTIFICATION_UNITS,
  NOTIFICATIONS_MAX,
} from "@/lib/event-form";

/** このフィールドを置くフォームが持っていなければならない値 */
export interface NotificationsFormValues {
  notifications: Notification[];
}

interface Props {
  label: string;
  /** 一覧の下に出す補足 */
  children?: React.ReactNode;
  /** 閲覧のみ (管理権限のないメンバーがサーバー設定を開いたとき) */
  disabled?: boolean;
}

/**
 * 「N 分前 / 時間前 / 日前 / 週間前」の通知一覧の入力 (最大 NOTIFICATIONS_MAX 件)。
 * 予定ダイアログ (`notifications`) とサーバー設定の「既定の事前通知」(#181) で同じ見た目にするため、
 * `FormProvider` 経由でフォームの `notifications` フィールドを扱う
 */
export function NotificationsField({ label, children, disabled }: Props) {
  const {
    control,
    register,
    formState: { errors },
  } = useFormContext<NotificationsFormValues>();
  const notifications = useFieldArray({ control, name: "notifications" });
  const listError =
    errors.notifications?.root?.message ?? errors.notifications?.message;

  return (
    <Field
      data-invalid={listError ? true : undefined}
      data-disabled={disabled || undefined}
    >
      <FieldLabel>{label}</FieldLabel>
      <div className="flex flex-col gap-2">
        {notifications.fields.map((item, index) => {
          const numError = errors.notifications?.[index]?.num;
          return (
            <div key={item.id} className="flex flex-wrap items-center gap-2">
              <Input
                type="number"
                inputMode="numeric"
                min={NOTIFICATION_NUM_MIN}
                max={NOTIFICATION_NUM_MAX}
                aria-label="通知のタイミング (数値)"
                aria-invalid={numError ? true : undefined}
                disabled={disabled}
                className="w-20"
                {...register(`notifications.${index}.num`, {
                  valueAsNumber: true,
                })}
              />
              <Controller
                control={control}
                name={`notifications.${index}.unit`}
                render={({ field }) => (
                  <Select
                    value={field.value}
                    onValueChange={(value) => {
                      if (value) field.onChange(value);
                    }}
                    items={NOTIFICATION_UNITS}
                    disabled={disabled}
                  >
                    <SelectTrigger
                      className="w-28"
                      aria-label="通知のタイミング (単位)"
                    >
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      {NOTIFICATION_UNITS.map((unit) => (
                        <SelectItem key={unit.value} value={unit.value}>
                          {unit.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label="この通知を削除"
                disabled={disabled}
                onClick={() => notifications.remove(index)}
              >
                <XIcon />
              </Button>
              <FieldError errors={[numError]} className="basis-full" />
            </div>
          );
        })}
        <div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={
              disabled || notifications.fields.length >= NOTIFICATIONS_MAX
            }
            onClick={() => notifications.append({ num: 1, unit: "hours" })}
          >
            <PlusIcon />
            通知を追加
          </Button>
        </div>
      </div>
      {children}
      <FieldError>{listError}</FieldError>
    </Field>
  );
}
