"use client";

import { zodResolver } from "@hookform/resolvers/zod";
import { InfoIcon, RefreshCwIcon } from "lucide-react";
import { useEffect, useState } from "react";
import {
  Controller,
  FormProvider,
  useForm,
  useFormContext,
} from "react-hook-form";
import { NotificationsField } from "@/components/form/notifications-field";
import { GuildDigestSettingsSection } from "@/components/guild-digest-settings";
import { SettingsHint } from "@/components/guild-settings/settings-hint";
import { useLanguage } from "@/components/language-provider";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldLabel,
} from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useLeaveConfirmation } from "@/hooks/use-leave-confirmation";
import { describeApiError } from "@/lib/api";
import type {
  GuildChannel,
  GuildConfig,
  NotificationPermission,
} from "@/lib/api/types";
import {
  channelLabel,
  describeMissingPermissions,
} from "@/lib/channel-permissions";
import {
  configToFormValues,
  formValuesToConfigInput,
  type GuildSettingsFormValues,
  guildSettingsSchema,
} from "@/lib/guild-settings-form";
import {
  useGuildChannelsQuery,
  useGuildConfigQuery,
  useGuildRolesQuery,
  useMyPermissionsQuery,
  useRefreshMyPermissions,
  useUpdateGuildConfig,
} from "@/lib/query/guild";

const RESTRICTED_CHECKBOX_ID = "guild-settings-restricted";
const NOTIFY_AT_START_CHECKBOX_ID = "guild-settings-notify-at-start";
const CHANNEL_SELECT_ID = "guild-settings-channel";

/** 設定を取れていないときのフォーム初期値 (通常は RSC で hydrate 済みなので使われない) */
const FALLBACK_CONFIG: Omit<GuildConfig, "guild_id"> = {
  locale: "ja",
  restricted: false,
  editor_role_ids: [],
  notify_at_start: true,
  default_notifications: [],
  notification_channel_id: null,
  notification_channel_configured: false,
};

/**
 * サーバー設定の「全般」(旧 ServerSetting.vue 相当): 投稿の言語、restricted モードと編集ロール、
 * Discord への通知 (#181)、予定のまとめ。まとめ以外は「保存」でまとめて反映する
 */
export function GuildGeneralSettings({ guildId }: { guildId: string }) {
  const { t, language } = useLanguage();
  const configQuery = useGuildConfigQuery(guildId);
  const permissionsQuery = useMyPermissionsQuery(guildId);
  const updateConfig = useUpdateGuildConfig(guildId);
  const canManage =
    !permissionsQuery.isError &&
    (permissionsQuery.data?.can_manage_server ?? false);
  const rolesQuery = useGuildRolesQuery(guildId, canManage);
  const refreshPermissions = useRefreshMyPermissions(guildId);
  const config = configQuery.data;
  const form = useForm<GuildSettingsFormValues>({
    resolver: zodResolver(guildSettingsSchema),
    defaultValues: configToFormValues({
      guild_id: guildId,
      ...FALLBACK_CONFIG,
      ...config,
    }),
  });
  const {
    control,
    handleSubmit,
    reset,
    formState: { isSubmitting, isDirty, dirtyFields },
  } = form;
  const [error, setError] = useState<string | Error | null>(null);
  const [saved, setSaved] = useState(false);
  // 未保存の変更があるまま別の節やカレンダーへ移ろうとしたら確認する (保存は節ごとなので、移ると消える)
  const leaveConfirmation = useLeaveConfirmation(isDirty);

  // 開くたびに設定を取り直す (staleTime 内だとマウントしただけでは再取得されない)。
  // 画面を開く前に `/init` や別のブラウザで変わっていた分を、古い値のまま保存で送り返さないため
  const { refetch: refetchConfig } = configQuery;
  useEffect(() => {
    void refetchConfig();
  }, [refetchConfig]);
  // 取り直した設定をフォームに反映する。まだ何も触っていないときだけ置き換え、編集中の入力は消さない
  // (編集後に届いた変更は「保存」で上書きされる。競合の解決までは持ち込まない)
  useEffect(() => {
    if (config && !isDirty) reset(configToFormValues(config));
  }, [config, isDirty, reset]);

  const reloading =
    permissionsQuery.isFetching ||
    refreshPermissions.isPending ||
    rolesQuery.isFetching;
  const saving = updateConfig.isPending || isSubmitting;
  // 開いたときの設定の取り直し (上の refetchConfig) が終わるまで、また取り直せなかったときは保存させない。
  // 古いキャッシュの値をそのまま送ると、別の経路で変わった通知先や既定の事前通知を取り消してしまう
  const syncing = configQuery.isFetching;
  const syncFailed = configQuery.isError;

  // Discord 側で権限を変えた後に押してもらう (旧 checkEditable)。入力内容はそのまま残す
  const reload = async () => {
    setError(null);
    try {
      const permissions = await refreshPermissions.mutateAsync();
      if (permissions.can_manage_server) {
        const result = await rolesQuery.refetch();
        if (result.error)
          setError(result.error instanceof Error ? result.error : new Error());
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause : new Error());
    }
  };

  // 保存後にまた編集を始めたら「保存しました」を消す
  useEffect(() => {
    if (isDirty) setSaved(false);
  }, [isDirty]);

  const save = handleSubmit(async (values) => {
    setError(null);
    setSaved(false);
    try {
      const rolesChanged = !!dirtyFields.editorRoleIds;
      if (
        rolesChanged &&
        (!rolesQuery.isSuccess ||
          values.editorRoleIds.some(
            (id) => !rolesQuery.data.some((role) => role.id === id),
          ))
      ) {
        setError(
          "ロール一覧を再取得し、削除されたロールを外してから保存してください",
        );
        return;
      }
      const updated = await updateConfig.mutateAsync(
        formValuesToConfigInput(values, rolesChanged),
      );
      reset(configToFormValues(updated));
      setSaved(true);
    } catch (cause) {
      setError(cause instanceof Error ? cause : new Error());
    }
  });

  return (
    <FormProvider {...form}>
      <form onSubmit={save} noValidate className="flex flex-col gap-4">
        <Field>
          <FieldLabel htmlFor="guild-locale">
            {t("サーバーの通知・投稿の言語")}
          </FieldLabel>
          <Controller
            control={control}
            name="locale"
            render={({ field }) => (
              <select
                id="guild-locale"
                value={field.value}
                onChange={field.onChange}
                disabled={!canManage || saving || syncing || syncFailed}
                className="h-11 rounded-md border border-border bg-background px-2"
              >
                <option value="ja" lang="ja">
                  日本語
                </option>
                <option value="en" lang="en">
                  English
                </option>
              </select>
            )}
          />
          <FieldDescription>
            {t(
              "チャンネル通知・まとめ・日程調整の投稿に使います。個人への返信とプッシュ通知の言語は変わりません。",
            )}
          </FieldDescription>
        </Field>

        <Field orientation="horizontal" data-disabled={!canManage || undefined}>
          <Controller
            control={control}
            name="restricted"
            render={({ field }) => (
              <Checkbox
                id={RESTRICTED_CHECKBOX_ID}
                checked={field.value}
                disabled={!canManage}
                onCheckedChange={(checked) => field.onChange(checked)}
              />
            )}
          />
          <FieldContent>
            <div className="flex items-center gap-1">
              <FieldLabel
                htmlFor={RESTRICTED_CHECKBOX_ID}
                className="font-normal"
              >
                {t(
                  "予定の編集を管理権限または指定ロールのあるメンバーに限定する",
                )}
              </FieldLabel>
              <SettingsHint label={t("予定の編集権限")}>
                {t(
                  "予定の追加・編集・削除を、サーバーのオーナーまたは「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限、または設定で指定したロールを持つメンバーに限定します。Discord 側で権限を変更したら「再読込」を押してください。変更後の権限とロール一覧を取り直します。",
                )}
              </SettingsHint>
            </div>
          </FieldContent>
        </Field>

        {canManage && (
          <Controller
            control={control}
            name="editorRoleIds"
            render={({ field }) => (
              <fieldset
                className="grid gap-2"
                disabled={!canManage || !form.watch("restricted") || saving}
              >
                <legend className="mb-2 text-sm font-medium">
                  {t("編集を許可するロール ({count}/25)", {
                    count: field.value.length,
                  })}
                </legend>
                <p className="text-sm text-muted-foreground">
                  {t(
                    "管理権限がなくても、選んだロールのメンバーは予定を追加・編集・削除できます",
                  )}
                </p>
                {rolesQuery.isPending ? (
                  <p className="text-sm">{t("ロールを読み込み中…")}</p>
                ) : rolesQuery.isError ? (
                  <p role="alert" className="text-sm text-destructive">
                    {t(
                      "ロール一覧を取得できませんでした。「再読込」で再試行してください。変更していないロール設定はそのまま保存されます",
                    )}
                  </p>
                ) : (
                  <div className="grid max-h-40 gap-2 overflow-y-auto rounded-md border p-3">
                    {[
                      ...rolesQuery.data,
                      ...field.value
                        .filter(
                          (id) =>
                            !rolesQuery.data.some((role) => role.id === id),
                        )
                        .map((id) => ({
                          id,
                          name: t("削除されたロール (ID: {id})", { id }),
                          color: 0,
                          position: 0,
                        })),
                    ].map((role) => (
                      <label
                        key={role.id}
                        htmlFor={`editor-role-${role.id}`}
                        className="flex items-center gap-2 text-sm"
                      >
                        <Checkbox
                          id={`editor-role-${role.id}`}
                          checked={field.value.includes(role.id)}
                          disabled={
                            !canManage ||
                            !form.watch("restricted") ||
                            saving ||
                            (field.value.length >= 25 &&
                              !field.value.includes(role.id))
                          }
                          onCheckedChange={(checked) =>
                            field.onChange(
                              checked
                                ? [...field.value, role.id]
                                : field.value.filter((id) => id !== role.id),
                            )
                          }
                        />
                        <span
                          aria-hidden
                          className="size-3 shrink-0 rounded-full border"
                          style={{
                            backgroundColor: role.color
                              ? `#${role.color.toString(16).padStart(6, "0")}`
                              : "var(--muted-foreground)",
                          }}
                        />
                        <span className="break-all">{role.name}</span>
                      </label>
                    ))}
                    {rolesQuery.data.length === 0 &&
                      field.value.length === 0 && (
                        <p className="text-sm text-muted-foreground">
                          {t("選択できるロールはありません")}
                        </p>
                      )}
                  </div>
                )}
              </fieldset>
            )}
          />
        )}

        <Separator />

        <NotificationSection
          guildId={guildId}
          canManage={canManage}
          currentChannelId={config?.notification_channel_id ?? null}
          configured={config?.notification_channel_configured ?? false}
        />

        <Separator />

        <GuildDigestSettingsSection
          guildId={guildId}
          canManage={canManage}
          configured={config?.notification_channel_configured}
        />

        {syncFailed && (
          <p role="alert" className="text-sm text-destructive">
            {t(
              "設定を取り直せませんでした ({error})。古い設定を上書きしないよう、保存はできません。ページを読み込み直してください",
              { error: describeApiError(configQuery.error, language) },
            )}
          </p>
        )}

        {error && (
          <div
            role="alert"
            className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive"
          >
            {typeof error === "string"
              ? t(error)
              : describeApiError(error, language)}
          </div>
        )}

        {saved && (
          <p role="status" className="text-sm text-muted-foreground">
            {t("保存しました")}
          </p>
        )}

        {/* 長いページの途中からでも押せるよう、下端に貼り付ける */}
        <div className="sticky -bottom-4 z-10 flex flex-col-reverse gap-2 border-t bg-background py-3 sm:-bottom-6 sm:flex-row sm:justify-end">
          <Button
            type="button"
            variant="outline"
            disabled={reloading || saving}
            onClick={reload}
            className="sm:mr-auto"
          >
            <RefreshCwIcon
              data-icon="inline-start"
              className={reloading ? "animate-spin" : undefined}
            />
            {reloading ? t("確認中…") : t("再読込")}
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={saving || !isDirty}
            onClick={() => {
              setError(null);
              if (config) reset(configToFormValues(config));
              else reset();
            }}
          >
            {t("変更を取り消す")}
          </Button>
          <Button
            type="submit"
            disabled={
              !canManage || saving || reloading || syncing || syncFailed
            }
          >
            {saving ? t("保存中…") : syncing ? t("確認中…") : t("保存")}
          </Button>
        </div>
      </form>

      {/* フォームの外に置く (ポータルでも React のイベントはフォームまで伝わり、送信を起こしうるため) */}
      <AlertDialog
        open={leaveConfirmation.pendingHref !== null}
        onOpenChange={(open) => {
          if (!open) leaveConfirmation.stay();
        }}
      >
        <AlertDialogContent lang={language}>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("未保存の変更を破棄しますか？")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                "「全般」の変更はまだ保存されていません。このページを離れると、変更は失われます。",
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("編集を続ける")}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={leaveConfirmation.leave}
            >
              {t("破棄して移動")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </FormProvider>
  );
}

/**
 * 通知の節 (#181): 通知先チャンネル (`/init` と同じ設定)、開始時刻の通知の ON/OFF、
 * 新しい予定の既定の事前通知。いずれも「保存」で反映する
 */
function NotificationSection({
  guildId,
  canManage,
  currentChannelId,
  configured,
}: {
  guildId: string;
  canManage: boolean;
  currentChannelId: string | null;
  /** 通知先が設定済みか (管理権限が無いと ID は返らないので、これで案内を出し分ける) */
  configured: boolean;
}) {
  const { t, language } = useLanguage();
  const { control } = useFormContext<GuildSettingsFormValues>();
  return (
    <section
      aria-labelledby="guild-settings-notifications"
      className="flex flex-col gap-4"
    >
      <div className="flex items-center gap-1">
        <h2
          id="guild-settings-notifications"
          className="text-base font-semibold"
        >
          {t("Discord への通知")}
        </h2>
        <SettingsHint label={t("Discord への通知")}>
          {t(
            "Bot が通知先チャンネルに予定の開始時刻と事前通知を投稿します。Discord の /init と同じ設定です。",
          )}
        </SettingsHint>
      </div>

      <div className="grid gap-4 sm:grid-cols-2 sm:items-start">
        <Controller
          control={control}
          name="notificationChannelId"
          render={({ field }) => (
            <ChannelField
              guildId={guildId}
              value={field.value}
              onChange={field.onChange}
              currentChannelId={currentChannelId}
              configured={configured}
              disabled={!canManage}
            />
          )}
        />

        <Field orientation="horizontal" data-disabled={!canManage || undefined}>
          <Controller
            control={control}
            name="notifyAtStart"
            render={({ field }) => (
              <Checkbox
                id={NOTIFY_AT_START_CHECKBOX_ID}
                checked={field.value}
                disabled={!canManage}
                onCheckedChange={(checked) => field.onChange(checked)}
              />
            )}
          />
          <FieldContent>
            <div className="flex items-center gap-1">
              <FieldLabel
                htmlFor={NOTIFY_AT_START_CHECKBOX_ID}
                className="font-normal"
              >
                {t("開始時刻に通知する")}
              </FieldLabel>
              <SettingsHint label={t("開始時刻の通知")}>
                {t(
                  "外すと、予定ごとに設定した事前通知だけが届きます。終日予定の 0:00 の通知も届きません。",
                )}
              </SettingsHint>
            </div>
          </FieldContent>
        </Field>
      </div>
      <NotificationsField
        label={t("既定の事前通知")}
        disabled={!canManage}
        language={language}
      >
        <FieldDescription>
          {t("新しい予定に使う初期値です。予定ごとに変更できます")}
        </FieldDescription>
      </NotificationsField>
    </section>
  );
}

/** 通知先チャンネルの選択。Bot が投稿できないチャンネルは選べず、足りない権限を添える */
function ChannelField({
  guildId,
  value,
  onChange,
  currentChannelId,
  configured,
  disabled,
}: {
  guildId: string;
  /** "" は未設定 */
  value: string;
  onChange: (value: string) => void;
  /** 保存済みの通知先。一覧に無い (スレッドや削除済みの) チャンネルでも選択肢として残す */
  currentChannelId: string | null;
  /** 通知先が設定済みか。管理権限が無いと ID (currentChannelId) は返らないので、これで表示を分ける */
  configured: boolean;
  disabled: boolean;
}) {
  const { t, language } = useLanguage();
  const channelsQuery = useGuildChannelsQuery(guildId);
  const channels = channelsQuery.data ?? [];
  // 一覧に無い保存済みのチャンネル (`/init` で設定したスレッドなど) は ID で表示して、
  // 他の設定を保存しても通知先が消えないようにする
  const unknownCurrent =
    currentChannelId !== null &&
    !channels.some((channel) => channel.id === currentChannelId)
      ? currentChannelId
      : null;
  const items = [
    ...channels.map((channel) => ({
      value: channel.id,
      label: channelLabel(channel),
    })),
    ...(unknownCurrent
      ? [
          {
            value: unknownCurrent,
            label: t("一覧にないチャンネル (ID: {id})", { id: unknownCurrent }),
          },
        ]
      : []),
  ];
  const groups = groupByCategory(channels);
  const selected = channels.find((channel) => channel.id === value);

  return (
    <TooltipProvider>
      <Field data-disabled={disabled || undefined}>
        <div className="flex items-center gap-1">
          <FieldLabel htmlFor={CHANNEL_SELECT_ID}>
            {t("通知先チャンネル")}
          </FieldLabel>
          <SettingsHint label={t("通知先チャンネル")}>
            {t(
              "Bot に「チャンネルを見る」「メッセージを送信」「埋め込みリンク」の権限があるテキストチャンネルを選べます。Discord 側でチャンネルや権限を変えた直後は、反映まで 1 分ほどかかることがあります。",
            )}
          </SettingsHint>
        </div>
        <Select
          value={value || null}
          onValueChange={(next) => onChange(next ?? "")}
          items={items}
          disabled={disabled || channelsQuery.isPending}
        >
          <SelectTrigger id={CHANNEL_SELECT_ID} className="w-full min-w-0">
            <SelectValue
              placeholder={
                channelsQuery.isPending
                  ? t("チャンネルを読み込み中…")
                  : configured
                    ? t(
                        "設定済み (管理権限を持つメンバーだけが確認・変更できます)",
                      )
                    : t("未設定 (通知は届きません)")
              }
            />
          </SelectTrigger>
          <SelectContent lang={language}>
            {groups.map((group) => (
              <SelectGroup key={group.category ?? ""}>
                {group.category !== null && (
                  <SelectLabel>{group.category}</SelectLabel>
                )}
                {group.channels.map((channel) => (
                  <SelectItem
                    key={channel.id}
                    value={channel.id}
                    disabled={!channel.can_post}
                  >
                    {channelLabel(channel)}
                    {!channel.can_post && (
                      <CannotPostHint
                        permissions={channel.missing_permissions}
                      />
                    )}
                  </SelectItem>
                ))}
              </SelectGroup>
            ))}
            {unknownCurrent && (
              <SelectItem value={unknownCurrent}>
                {t("一覧にないチャンネル (ID: {id})", { id: unknownCurrent })}
              </SelectItem>
            )}
          </SelectContent>
        </Select>
        {channelsQuery.isError ? (
          <FieldDescription className="text-destructive">
            {t(
              "チャンネルの一覧を取得できませんでした ({error})。通知先以外の設定は保存できます",
              { error: describeApiError(channelsQuery.error, language) },
            )}
          </FieldDescription>
        ) : value === "" && !channelsQuery.isPending && !configured ? (
          <FieldDescription>
            {t(
              "通知先が未設定のため、予定を作っても通知は届きません。チャンネルを選んで保存してください",
            )}
          </FieldDescription>
        ) : selected && !selected.can_post ? (
          <FieldDescription className="text-destructive">
            {t(
              "Bot に{permissions}の権限がないため、このチャンネルには通知を投稿できません。チャンネルの権限設定を見直すか、別のチャンネルを選んでください",
              {
                permissions: describeMissingPermissions(
                  selected.missing_permissions,
                  language,
                ),
              },
            )}
          </FieldDescription>
        ) : null}
      </Field>
    </TooltipProvider>
  );
}

/**
 * Bot が投稿できないチャンネルの理由 (足りない権限)。選択肢の中に文章で出すと幅に収まらず見切れるので、
 * ⓘ アイコンのツールチップに入れる。無効な選択肢は pointer-events を切られているので、
 * アイコンだけ受け取れるようにして hover で開く (読み上げには sr-only の文章を残す)
 */
function CannotPostHint({
  permissions,
}: {
  permissions: readonly NotificationPermission[];
}) {
  const { t, language } = useLanguage();
  const reason = t("Bot に{permissions}の権限がありません", {
    permissions: describeMissingPermissions(permissions, language),
  });
  return (
    // 選択肢の上に出るので、アイコンから離れたらすぐ閉じ (disableHoverablePopup)、ツールチップ自体は
    // ポインタイベントを受けない。残ったままだと重なった選択肢をクリックできない
    <Tooltip disableHoverablePopup>
      <TooltipTrigger
        render={
          <span className="pointer-events-auto inline-flex items-center text-muted-foreground" />
        }
      >
        <InfoIcon aria-hidden className="size-3.5" />
        <span className="sr-only">{reason}</span>
      </TooltipTrigger>
      <TooltipContent className="pointer-events-none">{reason}</TooltipContent>
    </Tooltip>
  );
}
/** Discord の表示順のままカテゴリごとにまとめる (カテゴリ無しは先頭) */
function groupByCategory(
  channels: readonly GuildChannel[],
): { category: string | null; channels: GuildChannel[] }[] {
  const groups: { category: string | null; channels: GuildChannel[] }[] = [];
  for (const channel of channels) {
    const last = groups.at(-1);
    if (last && last.category === channel.category) {
      last.channels.push(channel);
    } else {
      groups.push({ category: channel.category, channels: [channel] });
    }
  }
  return groups;
}
