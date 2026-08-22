"use client";

import { zodResolver } from "@hookform/resolvers/zod";
import { isBefore } from "date-fns";
import { PlusIcon, XIcon } from "lucide-react";
import { useState } from "react";
import { Controller, useFieldArray, useForm, useWatch } from "react-hook-form";
import { ColorPicker } from "@/components/form/color-picker";
import { DatePicker } from "@/components/form/date-picker";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { useLastValue } from "@/hooks/use-last-value";
import { describeApiError } from "@/lib/api";
import type { ApiEvent, ApiEventInput } from "@/lib/api/types";
import {
  DESCRIPTION_MAX_CHARS,
  type EventFormValues,
  eventFormSchema,
  eventFormToApiInput,
  eventToFormValues,
  NAME_MAX_CHARS,
  NOTIFICATION_NUM_MAX,
  NOTIFICATION_NUM_MIN,
  NOTIFICATION_UNITS,
  NOTIFICATIONS_MAX,
} from "@/lib/event-form";

export type EventDialogState =
  | { mode: "create"; values: EventFormValues }
  | { mode: "edit"; event: ApiEvent };

interface Props {
  /** null なら閉じている */
  state: EventDialogState | null;
  onClose: () => void;
  /** 保存。resolve したらダイアログを閉じ、reject されたらエラーを表示して開いたままにする */
  onSubmit: (input: ApiEventInput) => Promise<unknown>;
  /** 編集中の予定の削除ボタン (確認ダイアログは呼び出し側が出す) */
  onDelete: (event: ApiEvent) => void;
}

const NAME_INPUT_ID = "event-form-name";

/** 予定の作成・編集ダイアログ (旧 NewEvent.vue 相当) */
export function EventFormDialog({ state, onClose, onSubmit, onDelete }: Props) {
  // 閉じるアニメーションの間も直前の内容を出しておく
  const shown = useLastValue(state);
  return (
    <Dialog
      open={state !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      // 入力途中に外側をクリックしても閉じない (旧 v-dialog persistent)
      disablePointerDismissal
    >
      <DialogContent
        className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-2xl"
        initialFocus={() => document.getElementById(NAME_INPUT_ID)}
      >
        {shown && (
          <EventForm
            state={shown}
            onClose={onClose}
            onSubmit={onSubmit}
            onDelete={onDelete}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

interface FormProps extends Omit<Props, "state"> {
  state: EventDialogState;
}

// ダイアログが開くたびにマウントされる (Base UI の Dialog は閉じると Popup を unmount する) ので、
// useForm の defaultValues で初期値が決まる
function EventForm({ state, onClose, onSubmit, onDelete }: FormProps) {
  const isEdit = state.mode === "edit";
  const {
    control,
    register,
    handleSubmit,
    setValue,
    getValues,
    formState: { errors, isSubmitting },
  } = useForm<EventFormValues>({
    resolver: zodResolver(eventFormSchema),
    defaultValues: isEdit ? eventToFormValues(state.event) : state.values,
  });
  const notifications = useFieldArray({ control, name: "notifications" });
  const [isAllDay, name, description] = useWatch({
    control,
    name: ["isAllDay", "name", "description"],
  });
  const [submitError, setSubmitError] = useState<string | null>(null);

  // 開始日を終了日より後にしたら終了日も合わせる (旧フォームの onStartDateChanged)
  const handleStartDateChange = (date: Date) => {
    setValue("startDate", date, { shouldDirty: true, shouldValidate: true });
    if (isBefore(getValues("endDate"), date)) {
      setValue("endDate", date, { shouldDirty: true, shouldValidate: true });
    }
  };

  const submit = handleSubmit(async (values) => {
    setSubmitError(null);
    try {
      await onSubmit(eventFormToApiInput(values));
      onClose();
    } catch (error) {
      setSubmitError(describeApiError(error));
    }
  });

  const notificationsError =
    errors.notifications?.root?.message ?? errors.notifications?.message;

  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-5">
      <DialogHeader>
        <DialogTitle>{isEdit ? "予定を編集" : "予定を作成"}</DialogTitle>
        <DialogDescription className="sr-only">
          タイトル・日時・通知・色・説明を入力して
          {isEdit ? "保存" : "作成"}してください
        </DialogDescription>
      </DialogHeader>

      <FieldGroup className="gap-4">
        <Field data-invalid={errors.name ? true : undefined}>
          <FieldLabel htmlFor={NAME_INPUT_ID}>
            タイトル
            <span aria-hidden className="text-destructive">
              *
            </span>
          </FieldLabel>
          <Input
            id={NAME_INPUT_ID}
            placeholder="タイトルを入力"
            aria-invalid={errors.name ? true : undefined}
            {...register("name")}
          />
          <div className="flex items-start justify-between gap-2">
            <FieldError errors={[errors.name]} />
            <FieldDescription className="ml-auto shrink-0 text-xs">
              {charCount(name)}/{NAME_MAX_CHARS}
            </FieldDescription>
          </div>
        </Field>

        <div className="grid gap-4 sm:grid-cols-[1fr_8rem_auto]">
          <Field data-invalid={errors.startDate ? true : undefined}>
            <FieldLabel htmlFor="event-form-start-date">開始日</FieldLabel>
            <Controller
              control={control}
              name="startDate"
              render={({ field }) => (
                <DatePicker
                  id="event-form-start-date"
                  value={field.value}
                  onChange={handleStartDateChange}
                  invalid={!!errors.startDate}
                />
              )}
            />
            <FieldError errors={[errors.startDate]} />
          </Field>
          <Field data-invalid={errors.startTime ? true : undefined}>
            <FieldLabel htmlFor="event-form-start-time">開始時刻</FieldLabel>
            <Input
              id="event-form-start-time"
              type="time"
              disabled={isAllDay}
              aria-invalid={errors.startTime ? true : undefined}
              {...register("startTime")}
            />
            <FieldError errors={[errors.startTime]} />
          </Field>
          <Field orientation="horizontal" className="sm:mt-7 sm:w-auto">
            <Controller
              control={control}
              name="isAllDay"
              render={({ field }) => (
                <Checkbox
                  id="event-form-all-day"
                  checked={field.value}
                  onCheckedChange={(checked) => field.onChange(checked)}
                />
              )}
            />
            <FieldLabel htmlFor="event-form-all-day" className="font-normal">
              終日
            </FieldLabel>
          </Field>
        </div>

        <div className="grid gap-4 sm:grid-cols-[1fr_8rem_auto]">
          <Field data-invalid={errors.endDate ? true : undefined}>
            <FieldLabel htmlFor="event-form-end-date">終了日</FieldLabel>
            <Controller
              control={control}
              name="endDate"
              render={({ field }) => (
                <DatePicker
                  id="event-form-end-date"
                  value={field.value}
                  onChange={field.onChange}
                  invalid={!!errors.endDate}
                />
              )}
            />
            <FieldError errors={[errors.endDate]} />
          </Field>
          <Field data-invalid={errors.endTime ? true : undefined}>
            <FieldLabel htmlFor="event-form-end-time">終了時刻</FieldLabel>
            <Input
              id="event-form-end-time"
              type="time"
              disabled={isAllDay}
              aria-invalid={errors.endTime ? true : undefined}
              {...register("endTime")}
            />
            <FieldError errors={[errors.endTime]} />
          </Field>
          <Field data-invalid={errors.color ? true : undefined}>
            <FieldLabel htmlFor="event-form-color">色</FieldLabel>
            <Controller
              control={control}
              name="color"
              render={({ field }) => (
                <ColorPicker
                  id="event-form-color"
                  value={field.value}
                  onChange={field.onChange}
                  invalid={!!errors.color}
                  className="sm:w-36"
                />
              )}
            />
            <FieldError errors={[errors.color]} />
          </Field>
        </div>

        <Field data-invalid={notificationsError ? true : undefined}>
          <FieldLabel>通知</FieldLabel>
          <div className="flex flex-col gap-2">
            {notifications.fields.map((item, index) => {
              const numError = errors.notifications?.[index]?.num;
              return (
                <div
                  key={item.id}
                  className="flex flex-wrap items-center gap-2"
                >
                  <Input
                    type="number"
                    inputMode="numeric"
                    min={NOTIFICATION_NUM_MIN}
                    max={NOTIFICATION_NUM_MAX}
                    aria-label="通知のタイミング (数値)"
                    aria-invalid={numError ? true : undefined}
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
                disabled={notifications.fields.length >= NOTIFICATIONS_MAX}
                onClick={() => notifications.append({ num: 1, unit: "hours" })}
              >
                <PlusIcon />
                通知を追加
              </Button>
            </div>
          </div>
          <FieldError>{notificationsError}</FieldError>
        </Field>

        <Field data-invalid={errors.description ? true : undefined}>
          <FieldLabel htmlFor="event-form-description">説明</FieldLabel>
          <Textarea
            id="event-form-description"
            rows={3}
            aria-invalid={errors.description ? true : undefined}
            {...register("description")}
          />
          <div className="flex items-start justify-between gap-2">
            <FieldError errors={[errors.description]} />
            <FieldDescription className="ml-auto shrink-0 text-xs">
              {charCount(description)}/{DESCRIPTION_MAX_CHARS}
            </FieldDescription>
          </div>
        </Field>
      </FieldGroup>

      {submitError && (
        <div
          role="alert"
          className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive"
        >
          {submitError}
        </div>
      )}

      <DialogFooter>
        {isEdit && (
          <Button
            type="button"
            variant="destructive"
            disabled={isSubmitting}
            onClick={() => onDelete(state.event)}
            className="sm:mr-auto"
          >
            削除
          </Button>
        )}
        <Button
          type="button"
          variant="outline"
          disabled={isSubmitting}
          onClick={onClose}
        >
          キャンセル
        </Button>
        <Button type="submit" disabled={isSubmitting}>
          {isSubmitting ? "保存中…" : isEdit ? "保存" : "作成"}
        </Button>
      </DialogFooter>
    </form>
  );
}

/** API (Rust の chars().count()) と同じくコードポイント単位で数える */
function charCount(value: string | undefined): number {
  return value ? Array.from(value).length : 0;
}
