"use client";

import { zodResolver } from "@hookform/resolvers/zod";
import { CheckIcon, CopyIcon, RefreshCwIcon } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import {
  Controller,
  FormProvider,
  useForm,
  useFormContext,
} from "react-hook-form";
import { NotificationsField } from "@/components/form/notifications-field";
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
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
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
import { describeApiError } from "@/lib/api";
import type { GuildChannel, GuildConfig } from "@/lib/api/types";
import {
  channelLabel,
  describeMissingPermissions,
} from "@/lib/channel-permissions";
import { docPath } from "@/lib/docs";
import { buildFeedUrl } from "@/lib/feed-url";
import {
  configToFormValues,
  formValuesToConfigInput,
  type GuildSettingsFormValues,
  guildSettingsSchema,
} from "@/lib/guild-settings-form";
import {
  useGuildChannelsQuery,
  useGuildConfigQuery,
  useGuildFeedQuery,
  useIssueGuildFeed,
  useMyPermissionsQuery,
  useRevokeGuildFeed,
  useUpdateGuildConfig,
} from "@/lib/query/guild";

interface Props {
  guildId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const RESTRICTED_CHECKBOX_ID = "guild-settings-restricted";
const NOTIFY_AT_START_CHECKBOX_ID = "guild-settings-notify-at-start";
const CHANNEL_SELECT_ID = "guild-settings-channel";

/**
 * サーバー設定ダイアログ (旧 ServerSetting.vue 相当)。
 * restricted モードの切り替えと通知の設定 (#181) は「保存」で反映し、
 * iCal フィード (#95) の発行・無効化はその場で反映する
 */
export function GuildSettingsDialog({ guildId, open, onOpenChange }: Props) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      // 旧 v-dialog persistent と同じく外側クリックでは閉じない
      disablePointerDismissal
    >
      {/* フィードの節 (#95) が増えて背が高くなったので、低い画面ではダイアログ内でスクロールさせる (予定ダイアログと同じ) */}
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-lg">
        {/* Base UI の Dialog は閉じると Popup を unmount するので、開くたびに設定値から初期化される */}
        <GuildSettingsForm
          guildId={guildId}
          onClose={() => onOpenChange(false)}
        />
      </DialogContent>
    </Dialog>
  );
}

/** 設定を取れていないときのフォーム初期値 (通常は RSC で hydrate 済みなので使われない) */
const FALLBACK_CONFIG: Omit<GuildConfig, "guild_id"> = {
  restricted: false,
  notify_at_start: true,
  default_notifications: [],
  notification_channel_id: null,
  notification_channel_configured: false,
};

function GuildSettingsForm({
  guildId,
  onClose,
}: {
  guildId: string;
  onClose: () => void;
}) {
  const configQuery = useGuildConfigQuery(guildId);
  const permissionsQuery = useMyPermissionsQuery(guildId);
  const updateConfig = useUpdateGuildConfig(guildId);
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
    formState: { isSubmitting, isDirty },
  } = form;
  const [error, setError] = useState<string | null>(null);

  // 開くたびに設定を取り直す (staleTime 内だとマウントしただけでは再取得されない)。
  // ダイアログを開く前に `/init` や別のブラウザで変わっていた分を、古い値のまま保存で送り返さないため
  const { refetch: refetchConfig } = configQuery;
  useEffect(() => {
    void refetchConfig();
  }, [refetchConfig]);
  // 取り直した設定をフォームに反映する。まだ何も触っていないときだけ置き換え、編集中の入力は消さない
  // (編集後に届いた変更は「保存」で上書きされる。競合の解決までは持ち込まない)
  useEffect(() => {
    if (config && !isDirty) reset(configToFormValues(config));
  }, [config, isDirty, reset]);

  const canManage = permissionsQuery.data?.can_manage_server ?? false;
  const reloading = permissionsQuery.isFetching;
  const saving = updateConfig.isPending || isSubmitting;
  // 開いたときの設定の取り直し (下の refetchConfig) が終わるまで、また取り直せなかったときは保存させない。
  // 古いキャッシュの値をそのまま送ると、別の経路で変わった通知先や既定の事前通知を取り消してしまう
  const syncing = configQuery.isFetching;
  const syncFailed = configQuery.isError;

  // Discord 側で権限を変えた後に押してもらう (旧 checkEditable)。入力内容はそのまま残す
  const reload = async () => {
    setError(null);
    const result = await permissionsQuery.refetch();
    if (result.error) setError(describeApiError(result.error));
  };

  const save = handleSubmit(async (values) => {
    setError(null);
    try {
      await updateConfig.mutateAsync(formValuesToConfigInput(values));
      onClose();
    } catch (cause) {
      setError(describeApiError(cause));
    }
  });

  return (
    <FormProvider {...form}>
      <form onSubmit={save} noValidate className="flex flex-col gap-5">
        <DialogHeader>
          <DialogTitle>サーバー設定</DialogTitle>
          <DialogDescription>
            予定を編集できるユーザーの制限、Discord
            への通知、外部カレンダーからの購読を設定できます
          </DialogDescription>
        </DialogHeader>

        {permissionsQuery.data && !canManage && (
          <p className="rounded-md bg-muted px-3 py-2 text-sm text-muted-foreground">
            サーバーの設定の変更には「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限を持っている必要があります
          </p>
        )}

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
            <FieldLabel
              htmlFor={RESTRICTED_CHECKBOX_ID}
              className="font-normal"
            >
              予定の追加・編集・削除を「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限を持ったユーザーに限定する
            </FieldLabel>
          </FieldContent>
        </Field>

        <FieldDescription>
          Discord
          側でユーザーの権限を変更した場合は「再読込」を押してください。反映まで最大
          1 分ほどかかることがあります
        </FieldDescription>

        <Separator />

        <NotificationSection
          guildId={guildId}
          canManage={canManage}
          currentChannelId={config?.notification_channel_id ?? null}
          configured={config?.notification_channel_configured ?? false}
        />

        <Separator />

        <FeedSection
          guildId={guildId}
          canManage={canManage}
          onError={setError}
        />

        {syncFailed && (
          <p role="alert" className="text-sm text-destructive">
            設定を取り直せませんでした ({describeApiError(configQuery.error)}
            )。古い設定を上書きしないよう、保存はできません。ダイアログを開き直してください
          </p>
        )}

        {error && (
          <div
            role="alert"
            className="rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive"
          >
            {error}
          </div>
        )}

        <DialogFooter>
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
            {reloading ? "確認中…" : "再読込"}
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={saving}
            onClick={onClose}
          >
            キャンセル
          </Button>
          <Button
            type="submit"
            disabled={
              !canManage || saving || reloading || syncing || syncFailed
            }
          >
            {saving ? "保存中…" : syncing ? "確認中…" : "保存"}
          </Button>
        </DialogFooter>
      </form>
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
  const { control } = useFormContext<GuildSettingsFormValues>();
  return (
    <section
      aria-labelledby="guild-settings-notifications"
      className="flex flex-col gap-4"
    >
      <div className="flex flex-col gap-1">
        <h3 id="guild-settings-notifications" className="text-sm font-medium">
          Discord への通知
        </h3>
        <p className="text-sm text-muted-foreground">
          予定の開始時刻と事前通知のタイミングに、Bot
          がここで選んだチャンネルへ通知を投稿します。Discord の{" "}
          <code className="rounded bg-muted px-1 font-mono text-xs">/init</code>{" "}
          で設定した通知先と同じ設定です
        </p>
      </div>

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
          <FieldLabel
            htmlFor={NOTIFY_AT_START_CHECKBOX_ID}
            className="font-normal"
          >
            予定の開始時刻に通知する
          </FieldLabel>
          <FieldDescription>
            外すと、予定ごとに設定した事前通知だけが届きます (終日予定の 0:00
            の通知も届きません)
          </FieldDescription>
        </FieldContent>
      </Field>

      <NotificationsField
        label="新しい予定の既定の事前通知"
        disabled={!canManage}
      >
        <FieldDescription>
          このサーバーで予定を新しく作るときの「通知」の初期値です。予定ごとに変更できます
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
            label: `一覧にないチャンネル (ID: ${unknownCurrent})`,
          },
        ]
      : []),
  ];
  const groups = groupByCategory(channels);
  const selected = channels.find((channel) => channel.id === value);

  return (
    <Field data-disabled={disabled || undefined}>
      <FieldLabel htmlFor={CHANNEL_SELECT_ID}>通知先チャンネル</FieldLabel>
      <Select
        value={value || null}
        onValueChange={(next) => onChange(next ?? "")}
        items={items}
        disabled={disabled || channelsQuery.isPending}
      >
        <SelectTrigger id={CHANNEL_SELECT_ID} className="w-full sm:w-72">
          <SelectValue
            placeholder={
              channelsQuery.isPending
                ? "チャンネルを読み込み中…"
                : configured
                  ? "設定済み (管理権限を持つメンバーだけが確認・変更できます)"
                  : "未設定 (通知は届きません)"
            }
          />
        </SelectTrigger>
        <SelectContent>
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
                    <span className="text-xs text-muted-foreground">
                      Bot に
                      {describeMissingPermissions(channel.missing_permissions)}
                      の権限がありません
                    </span>
                  )}
                </SelectItem>
              ))}
            </SelectGroup>
          ))}
          {unknownCurrent && (
            <SelectItem value={unknownCurrent}>
              一覧にないチャンネル (ID: {unknownCurrent})
            </SelectItem>
          )}
        </SelectContent>
      </Select>
      {channelsQuery.isError ? (
        <FieldDescription className="text-destructive">
          チャンネルの一覧を取得できませんでした (
          {describeApiError(channelsQuery.error)}
          )。通知先以外の設定は保存できます
        </FieldDescription>
      ) : value === "" && !channelsQuery.isPending && !configured ? (
        <FieldDescription>
          通知先が未設定のため、予定を作っても通知は届きません。チャンネルを選んで保存してください
        </FieldDescription>
      ) : selected && !selected.can_post ? (
        <FieldDescription className="text-destructive">
          Bot に{describeMissingPermissions(selected.missing_permissions)}
          の権限がないため、このチャンネルには通知を投稿できません。チャンネルの権限設定を見直すか、別のチャンネルを選んでください
        </FieldDescription>
      ) : (
        <FieldDescription>
          Bot
          に「チャンネルを見る」「メッセージを送信」「埋め込みリンク」の権限があるテキストチャンネルを選べます。Discord
          側でチャンネルや権限を変えた直後は、反映まで 1
          分ほどかかることがあります
        </FieldDescription>
      )}
    </Field>
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
/** 再発行と無効化は取り消せないので、実行前に確認する */
type FeedConfirmation = "reissue" | "revoke";

/**
 * iCal フィード (#95) の節。URL の閲覧・コピーはメンバー全員、発行・再発行・無効化は管理権限を持つ人だけ
 * (API 側でも同じ条件で 403 になる)。「保存」とは独立に、押した時点で API を呼んで反映する
 */
function FeedSection({
  guildId,
  canManage,
  onError,
}: {
  guildId: string;
  canManage: boolean;
  onError: (message: string | null) => void;
}) {
  const feedQuery = useGuildFeedQuery(guildId);
  const issueFeed = useIssueGuildFeed(guildId);
  const revokeFeed = useRevokeGuildFeed(guildId);
  const [confirmation, setConfirmation] = useState<FeedConfirmation | null>(
    null,
  );
  const [copied, setCopied] = useState(false);
  // URL はこのページを開いているオリジンで組み立てる (ローカル / staging / 本番のどれでも設定なしで合う)。
  // ダイアログの中身は開いたときにだけ描画されるので、ここで window を読んでも SSR とずれない
  const [origin] = useState(() =>
    typeof window === "undefined" ? "" : window.location.origin,
  );

  const busy = issueFeed.isPending || revokeFeed.isPending;
  const feed = feedQuery.data;
  const url = feed ? buildFeedUrl(origin, feed.token) : null;

  // 「コピーしました」は数秒で元の表示に戻す
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(timer);
  }, [copied]);

  const run = async (action: () => Promise<unknown>) => {
    onError(null);
    try {
      await action();
    } catch (cause) {
      onError(describeApiError(cause));
    }
  };

  const copy = async () => {
    if (!url) return;
    onError(null);
    try {
      await navigator.clipboard.writeText(url);
      setCopied(true);
    } catch {
      // 権限が無い / 非セキュアなコンテキストなど。入力欄は選択できるので手動コピーを案内する
      onError("コピーできませんでした。URL を選択してコピーしてください");
    }
  };

  const confirm = () => {
    const action = confirmation;
    setConfirmation(null);
    if (action === "reissue") void run(() => issueFeed.mutateAsync());
    if (action === "revoke") void run(() => revokeFeed.mutateAsync());
  };

  return (
    <section
      aria-labelledby="guild-settings-feed"
      className="flex flex-col gap-3"
    >
      <div className="flex flex-col gap-1">
        <h3 id="guild-settings-feed" className="text-sm font-medium">
          外部カレンダーで購読する
        </h3>
        <p className="text-sm text-muted-foreground">
          このサーバーの予定を Google カレンダーや Apple
          カレンダーなどに表示するための URL です。URL
          を知っている人は誰でも予定を読めるので、共有する相手にご注意ください
        </p>
      </div>

      {feedQuery.isPending ? (
        <p className="text-sm text-muted-foreground">確認中…</p>
      ) : feedQuery.isError ? (
        <p role="alert" className="text-sm text-destructive">
          {describeApiError(feedQuery.error)}
        </p>
      ) : feed && url ? (
        <>
          <div className="flex gap-2">
            <Input
              readOnly
              value={url}
              aria-label="フィード URL"
              onFocus={(event) => event.currentTarget.select()}
              className="font-mono text-xs"
            />
            <Button
              type="button"
              variant="outline"
              onClick={copy}
              className="shrink-0"
            >
              {copied ? (
                <CheckIcon data-icon="inline-start" />
              ) : (
                <CopyIcon data-icon="inline-start" />
              )}
              {copied ? "コピーしました" : "コピー"}
            </Button>
          </div>
          {canManage && (
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={busy}
                onClick={() => setConfirmation("reissue")}
              >
                {issueFeed.isPending ? "再発行中…" : "再発行"}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={busy}
                onClick={() => setConfirmation("revoke")}
              >
                {revokeFeed.isPending ? "無効化中…" : "無効化"}
              </Button>
            </div>
          )}
        </>
      ) : canManage ? (
        <div>
          <Button
            type="button"
            variant="outline"
            disabled={busy}
            onClick={() => void run(() => issueFeed.mutateAsync())}
          >
            {issueFeed.isPending ? "発行中…" : "フィード URL を発行"}
          </Button>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">
          まだ発行されていません。管理権限を持つメンバーがこの画面から発行できます
        </p>
      )}

      <FieldDescription>
        カレンダーアプリへの登録のしかたは使い方の
        <Link
          href={docPath("subscribe")}
          target="_blank"
          rel="noreferrer"
          className="underline underline-offset-2"
        >
          「外部カレンダーで見る」
        </Link>
        を参照してください。反映までの時間は各カレンダーサービスの更新間隔によります
      </FieldDescription>

      <AlertDialog
        open={confirmation !== null}
        onOpenChange={(open) => {
          if (!open) setConfirmation(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirmation === "revoke"
                ? "フィード URL を無効化しますか?"
                : "フィード URL を再発行しますか?"}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {confirmation === "revoke"
                ? "今の URL は使えなくなり、購読しているカレンダーには予定が届かなくなります。もう一度使うには改めて発行します"
                : "新しい URL に置き換わり、今の URL は使えなくなります。購読している人には新しい URL を登録し直してもらってください"}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>キャンセル</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={confirm}>
              {confirmation === "revoke" ? "無効化" : "再発行"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
