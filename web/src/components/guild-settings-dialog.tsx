"use client";

import { CheckIcon, CopyIcon, RefreshCwIcon } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
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
import { Separator } from "@/components/ui/separator";
import { describeApiError } from "@/lib/api";
import { docPath } from "@/lib/docs";
import { buildFeedUrl } from "@/lib/feed-url";
import {
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

/**
 * サーバー設定ダイアログ (旧 ServerSetting.vue 相当)。
 * restricted モードの切り替え (「保存」で反映) と、iCal フィード (#95) の発行・無効化 (その場で反映)
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
  const [restricted, setRestricted] = useState(
    configQuery.data?.restricted ?? false,
  );
  const [error, setError] = useState<string | null>(null);

  const canManage = permissionsQuery.data?.can_manage_server ?? false;
  const reloading = permissionsQuery.isFetching;
  const saving = updateConfig.isPending;

  // Discord 側で権限を変えた後に押してもらう (旧 checkEditable)。チェック内容はそのまま残す
  const reload = async () => {
    setError(null);
    const result = await permissionsQuery.refetch();
    if (result.error) setError(describeApiError(result.error));
  };

  const save = async () => {
    setError(null);
    try {
      await updateConfig.mutateAsync(restricted);
      onClose();
    } catch (cause) {
      setError(describeApiError(cause));
    }
  };

  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
      className="flex flex-col gap-5"
    >
      <DialogHeader>
        <DialogTitle>サーバー設定</DialogTitle>
        <DialogDescription>
          予定を編集できるユーザーの制限と、外部カレンダーからの購読を設定できます
        </DialogDescription>
      </DialogHeader>

      {permissionsQuery.data && !canManage && (
        <p className="rounded-md bg-muted px-3 py-2 text-sm text-muted-foreground">
          サーバーの設定の変更には「管理者」「サーバー管理」「ロールの管理」「メッセージの管理」のいずれかの権限を持っている必要があります
        </p>
      )}

      <Field orientation="horizontal" data-disabled={!canManage || undefined}>
        <Checkbox
          id={RESTRICTED_CHECKBOX_ID}
          checked={restricted}
          disabled={!canManage}
          onCheckedChange={(checked) => setRestricted(checked)}
        />
        <FieldContent>
          <FieldLabel htmlFor={RESTRICTED_CHECKBOX_ID} className="font-normal">
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

      <FeedSection guildId={guildId} canManage={canManage} onError={setError} />

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
        <Button type="submit" disabled={!canManage || saving || reloading}>
          {saving ? "保存中…" : "保存"}
        </Button>
      </DialogFooter>
    </form>
  );
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
