"use client";

import { zodResolver } from "@hookform/resolvers/zod";
import { addDays, isBefore } from "date-fns";
import { useEffect, useRef, useState } from "react";
import {
  type Control,
  Controller,
  FormProvider,
  useForm,
  useWatch,
} from "react-hook-form";
import { EventShareControls } from "@/components/event-share-controls";
import { ColorPicker } from "@/components/form/color-picker";
import { DatePicker } from "@/components/form/date-picker";
import { NotificationMentionsField } from "@/components/form/notification-mentions-field";
import { NotificationsField } from "@/components/form/notifications-field";
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
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { useLastValue } from "@/hooks/use-last-value";
import { describeApiError } from "@/lib/api";
import type { ApiEvent, ApiEventInput } from "@/lib/api/types";
import { nowInJst } from "@/lib/calendar-events";
import {
  DESCRIPTION_MAX_CHARS,
  type EventFormValues,
  eventFormSchema,
  eventFormToApiInput,
  eventToFormValues,
  formStartAt,
  NAME_MAX_CHARS,
  withCheckedDiscordEvent,
} from "@/lib/event-form";

export type EventDialogState =
  | { mode: "create"; values: EventFormValues }
  | { mode: "edit"; event: ApiEvent };

interface Props {
  mentionGuildId?: string;
  /** null なら閉じている */
  state: EventDialogState | null;
  /** 通常のサーバーカレンダーで、編集権限があるときだけ共有操作を出す。 */
  allowShare?: boolean;
  onClose: () => void;
  /** 保存。resolve したらダイアログを閉じ、reject されたらエラーを表示して開いたままにする */
  onSubmit: (input: ApiEventInput) => Promise<unknown>;
  /** 編集中の予定の削除ボタン (確認ダイアログは呼び出し側が出す) */
  onDelete: (event: ApiEvent) => void;
  /**
   * 「Discord のイベントとしても作成する」(#94) の表示設定。
   * 未指定 (管理コンソールなど連携を扱わない画面) ならチェックボックス自体を出さない
   */
  discordSync?: {
    /** Bot 自身が「イベントの作成」権限を持つか。false なら無効化して案内を出す */
    botCreateEvents: boolean;
    /**
     * このユーザー自身が Discord の「イベントの作成」権限を持つか。
     * false なら新たな連携はできない (api も 403 で拒否する)
     */
    canCreateEvents: boolean;
    /**
     * 権限を取り直す (#122)。api 側は権限を数分キャッシュするので、Bot を招待し直したり
     * ロールを付けてもらった直後は上の 2 つが古いままになる。
     * 渡すと、権限不足で使えないときに「権限を再確認」ボタンを出す
     */
    onRefresh?: () => Promise<unknown>;
  };
}

const NAME_INPUT_ID = "event-form-name";

/** 予定の作成・編集ダイアログ (旧 NewEvent.vue 相当) */
export function EventFormDialog({
  state,
  onClose,
  onSubmit,
  onDelete,
  discordSync,
  allowShare,
  mentionGuildId,
}: Props) {
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
            discordSync={discordSync}
            allowShare={allowShare}
            mentionGuildId={mentionGuildId}
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
function EventForm({
  state,
  onClose,
  onSubmit,
  onDelete,
  discordSync,
  allowShare,
  mentionGuildId,
}: FormProps) {
  const isEdit = state.mode === "edit";
  const initialValues = isEdit ? eventToFormValues(state.event) : state.values;
  const form = useForm<EventFormValues>({
    resolver: zodResolver(eventFormSchema),
    // 連携を扱わない画面 (管理コンソール) ではチェックボックスを出さないので、値も落とす。
    // 連携済みの予定を開くと `eventToFormValues` が true にするが、そのままだと
    // 見えないフラグで Discord 向けの検証だけが効いて保存できなくなる
    defaultValues: discordSync
      ? initialValues
      : { ...initialValues, discordEvent: false },
  });
  const {
    control,
    register,
    handleSubmit,
    setValue,
    getValues,
    formState: { errors, isSubmitting },
  } = form;
  const [isAllDay, name, description, startDate, startTime] = useWatch({
    control,
    name: ["isAllDay", "name", "description", "startDate", "startTime"],
  });
  const endDateBeforeRollover = useRef<Date | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);

  // Discord 連携 (#94): 開始が過去 (現在を含む) だと Discord はイベントを作れない。
  // api 側の validate_discord_flag と同じ条件・同じ JST の現在時刻でチェックを無効化する
  const startAt =
    startDate instanceof Date
      ? formStartAt({ isAllDay, startDate, startTime })
      : null;
  const discordStartsInPast =
    startAt !== null && startAt.getTime() <= nowInJst().getTime();
  // 連携を**新しく作る**には Bot と本人の両方に権限が要る。
  // 既に連携済みの予定を編集しているときだけは、権限が無くてもチェックを外せる
  // (解除の出口まで塞がないため)。複製は連携済みの値を引き継いだ「新規作成」なので、
  // ここには含めない (含めると権限のない人が送信できてしまい、保存時に 403 になる)
  const isLinkedEdit =
    state.mode === "edit" && state.event.discord_scheduled_event_id !== null;
  const discordLocked =
    !isLinkedEdit &&
    discordSync !== undefined &&
    (!discordSync.botCreateEvents || !discordSync.canCreateEvents);
  useEffect(() => {
    // 無効化したら値も落とす (表示と送信値を一致させる)。連携済みの予定なら保存時に解除される
    if ((discordStartsInPast || discordLocked) && getValues("discordEvent")) {
      setValue("discordEvent", false, { shouldDirty: true });
    }
  }, [discordStartsInPast, discordLocked, getValues, setValue]);

  // 開始日を終了日より後にしたら終了日も合わせる (旧フォームの onStartDateChanged)
  const handleStartDateChange = (date: Date) => {
    endDateBeforeRollover.current = null;
    setValue("startDate", date, { shouldDirty: true, shouldValidate: true });
    if (isBefore(getValues("endDate"), date)) {
      setValue("endDate", date, { shouldDirty: true, shouldValidate: true });
    }
  };

  const submit = handleSubmit(async (values) => {
    setSubmitError(null);
    try {
      // 開いたまま開始時刻をまたぐことがあるので、連携の可否は送信直前にも確かめる
      const input = eventFormToApiInput(withCheckedDiscordEvent(values));
      // 日時だけの編集や管理画面では、保存済みのメンション先をそのまま維持する。
      if (
        !mentionGuildId ||
        (isEdit && !form.formState.dirtyFields.notificationMentions)
      ) {
        delete input.notification_mentions;
      }
      await onSubmit(input);
      onClose();
    } catch (error) {
      setSubmitError(describeApiError(error));
    }
  });

  return (
    <FormProvider {...form}>
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

          {/* 開始行 (開始日・開始時刻・終日) と終了行 (終了日・終了時刻・色) で
            列の位置が揃うよう、1 つのグリッドで 2 行に並べる */}
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
                    onCheckedChange={(checked) => {
                      // 終日から始めたフォームの既定時刻だけ繰り越す。時間指定の既存日時には加算しない。
                      if (
                        !checked &&
                        initialValues.isAllDay &&
                        getValues("endTime") < getValues("startTime")
                      ) {
                        endDateBeforeRollover.current = getValues("endDate");
                        setValue("endDate", addDays(getValues("endDate"), 1), {
                          shouldDirty: true,
                        });
                      }
                      if (checked && endDateBeforeRollover.current) {
                        // 日付を手で変えていなければ、自動補完だけを取り消す。
                        setValue("endDate", endDateBeforeRollover.current, {
                          shouldDirty: true,
                        });
                        endDateBeforeRollover.current = null;
                      }
                      field.onChange(checked);
                    }}
                  />
                )}
              />
              <FieldLabel htmlFor="event-form-all-day" className="font-normal">
                終日
              </FieldLabel>
            </Field>
            <Field data-invalid={errors.endDate ? true : undefined}>
              <FieldLabel htmlFor="event-form-end-date">終了日</FieldLabel>
              <Controller
                control={control}
                name="endDate"
                render={({ field }) => (
                  <DatePicker
                    id="event-form-end-date"
                    value={field.value}
                    onChange={(date) => {
                      endDateBeforeRollover.current = null;
                      field.onChange(date);
                    }}
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

          {/* 通知の一覧はサーバー設定の「既定の事前通知」(#181) と同じ部品 */}
          <NotificationsField label="通知" />
          {mentionGuildId && (
            <NotificationMentionsField guildId={mentionGuildId} />
          )}

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

          {discordSync && (
            <DiscordEventField
              control={control}
              isLinkedEdit={isLinkedEdit}
              botCreateEvents={discordSync.botCreateEvents}
              canCreateEvents={discordSync.canCreateEvents}
              startsInPast={discordStartsInPast}
              onRefresh={discordSync.onRefresh}
            />
          )}
        </FieldGroup>

        {isEdit && allowShare && (
          <EventShareControls key={state.event.id} event={state.event} />
        )}

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
    </FormProvider>
  );
}

/**
 * 「Discord のイベントとしても作成する」(#94)。
 * Bot か自分に「イベントの作成」権限が無いときは新たに有効にはできないが、
 * **連携済みの予定を編集しているとき**は外すことができる
 * (解除まで塞ぐと連携をやめる手段が無くなるため)。
 * 連携済みの予定の複製は「チェック済みの新規作成」なので、この例外には当たらない
 */
function DiscordEventField({
  control,
  isLinkedEdit,
  botCreateEvents,
  canCreateEvents,
  startsInPast,
  onRefresh,
}: {
  control: Control<EventFormValues>;
  isLinkedEdit: boolean;
  botCreateEvents: boolean;
  canCreateEvents: boolean;
  startsInPast: boolean;
  onRefresh?: () => Promise<unknown>;
}) {
  const locked =
    startsInPast || (!isLinkedEdit && (!botCreateEvents || !canCreateEvents));
  // 権限が足りないときだけ取り直せるようにする (#122)。過去開始は待っても変わらないので出さない
  const missingPermission = !botCreateEvents || !canCreateEvents;
  return (
    <Field orientation="horizontal" data-disabled={locked || undefined}>
      <Controller
        control={control}
        name="discordEvent"
        render={({ field }) => (
          <Checkbox
            id="event-form-discord-event"
            checked={field.value}
            disabled={locked}
            onCheckedChange={(value) => field.onChange(value)}
          />
        )}
      />
      <FieldContent>
        <FieldLabel htmlFor="event-form-discord-event" className="font-normal">
          Discord のイベントとしても作成する
        </FieldLabel>
        <FieldDescription>
          {discordEventHint({
            isLinkedEdit,
            botCreateEvents,
            canCreateEvents,
            startsInPast,
          })}
        </FieldDescription>
        {onRefresh && missingPermission && !startsInPast && (
          <RefreshPermissionsButton onRefresh={onRefresh} />
        )}
      </FieldContent>
    </Field>
  );
}

/**
 * 「権限を再確認」ボタン (#122)。
 *
 * api は Discord の権限を数分キャッシュするので、案内どおりに Bot を招待し直したり
 * ロールを付けてもらっても、そのままでは反映されるまで待たされる。
 * 押すとキャッシュを捨てて取り直し、権限が付いていればチェックボックスがその場で使えるようになる
 * (使えるようになるとこのボタン自体が消える)
 */
function RefreshPermissionsButton({
  onRefresh,
}: {
  onRefresh: () => Promise<unknown>;
}) {
  const [state, setState] = useState<
    "idle" | "pending" | "unchanged" | "error"
  >("idle");
  return (
    <div className="mt-1 flex flex-wrap items-center gap-2">
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={state === "pending"}
        onClick={async () => {
          setState("pending");
          try {
            await onRefresh();
            // 権限が付いていれば呼び出し元の再描画でこのボタンは消える。
            // 残っているということは Discord 側がまだ変わっていない
            setState("unchanged");
          } catch {
            setState("error");
          }
        }}
      >
        {state === "pending" ? "確認中…" : "権限を再確認"}
      </Button>
      {state === "unchanged" && (
        <span className="text-sm text-muted-foreground">
          Discord 側の権限はまだ変わっていません
        </span>
      )}
      {state === "error" && (
        <span className="text-sm text-destructive">
          確認できませんでした。時間をおいて試してください
        </span>
      )}
    </div>
  );
}

/** チェックボックスの下に出す案内。無効化の理由 (過去開始 / 権限不足) を伝える */
function discordEventHint({
  isLinkedEdit,
  botCreateEvents,
  canCreateEvents,
  startsInPast,
}: {
  isLinkedEdit: boolean;
  botCreateEvents: boolean;
  canCreateEvents: boolean;
  startsInPast: boolean;
}) {
  if (startsInPast) {
    return "開始日時が過去の予定は Discord のイベントにできません (連携済みの予定は保存すると連携が解除されます)";
  }
  // 自分の権限不足は Discord 側の設定次第なので、Bot の再招待を案内しても直らない
  if (!canCreateEvents) {
    return isLinkedEdit
      ? "あなたに Discord の「イベントの作成」権限がないため、この連携を作り直すことはできません。チェックを外すと連携を解除します"
      : "Discord の「イベントの作成」権限を持つ人だけが利用できます。サーバーの管理者にロールの権限を確認してください";
  }
  if (!botCreateEvents) {
    return isLinkedEdit
      ? "Bot に「イベントの作成」権限がないため、変更は Discord に反映できません。チェックを外すと連携を解除します"
      : botPermissionHint;
  }
  return "予定の作成・変更・削除を Discord のスケジュールイベントにも反映します";
}

/** Bot に権限がないときの案内 (再招待への導線つき) */
const botPermissionHint = (
  <>
    Bot に「イベントの作成」権限がないため利用できません。
    <a
      href="/docs/invite"
      target="_blank"
      rel="noreferrer"
      className="underline underline-offset-2"
    >
      Bot を招待し直す
    </a>
    と利用できます
  </>
);

/** API (Rust の chars().count()) と同じくコードポイント単位で数える */
function charCount(value: string | undefined): number {
  return value ? Array.from(value).length : 0;
}
