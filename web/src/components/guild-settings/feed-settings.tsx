"use client";

import { CheckIcon, CopyIcon } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
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
import { FieldDescription } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { describeApiError } from "@/lib/api";
import { docPath } from "@/lib/docs";
import { buildFeedUrl } from "@/lib/feed-url";
import {
  useGuildFeedQuery,
  useIssueGuildFeed,
  useRevokeGuildFeed,
} from "@/lib/query/guild";

/** 再発行と無効化は取り消せないので、実行前に確認する */
type FeedConfirmation = "reissue" | "revoke";

/**
 * iCal フィード (#95) の節。URL の閲覧・コピーはメンバー全員、発行・再発行・無効化は管理権限を持つ人だけ
 * (API 側でも同じ条件で 403 になる)。押した時点で API を呼んで反映する
 */
export function FeedSettings({
  guildId,
  canManage,
}: {
  guildId: string;
  canManage: boolean;
}) {
  const { t, language } = useLanguage();
  const [error, onError] = useState<string | Error | null>(null);
  const feedQuery = useGuildFeedQuery(guildId);
  const issueFeed = useIssueGuildFeed(guildId);
  const revokeFeed = useRevokeGuildFeed(guildId);
  const [confirmation, setConfirmation] = useState<FeedConfirmation | null>(
    null,
  );
  const [copied, setCopied] = useState(false);
  // URL はこのページを開いているオリジンで組み立てる (ローカル / staging / 本番のどれでも設定なしで合う)。
  // フィードの発行状況はブラウザでだけ取得する (SSR では「確認中…」) ので、URL を描画するのは常にブラウザ側。
  // ここで window を読んでも SSR の出力とはずれない
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
      onError(cause instanceof Error ? cause : new Error());
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
      className="flex flex-col gap-2"
    >
      <div className="flex flex-col gap-1">
        <h2 id="guild-settings-feed" className="text-base font-semibold">
          {t("外部カレンダーで購読する")}
        </h2>
        <p className="text-sm text-muted-foreground">
          {t(
            "URL を知っている人は誰でも予定を読めます。共有する相手にご注意ください",
          )}
        </p>
      </div>

      {feedQuery.isPending ? (
        <p className="text-sm text-muted-foreground">{t("確認中…")}</p>
      ) : feedQuery.isError ? (
        <p role="alert" className="text-sm text-destructive">
          {describeApiError(feedQuery.error, language)}
        </p>
      ) : feed && url ? (
        <>
          <div className="flex gap-2">
            <Input
              readOnly
              value={url}
              aria-label={t("フィード URL")}
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
              {copied ? t("コピーしました") : t("コピー")}
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
                {issueFeed.isPending ? t("再発行中…") : t("再発行")}
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={busy}
                onClick={() => setConfirmation("revoke")}
              >
                {revokeFeed.isPending ? t("無効化中…") : t("無効化")}
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
            {issueFeed.isPending ? t("発行中…") : t("フィード URL を発行")}
          </Button>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">
          {t(
            "まだ発行されていません。管理権限を持つメンバーがこの画面から発行できます",
          )}
        </p>
      )}

      {error && (
        <p role="alert" className="text-sm text-destructive">
          {typeof error === "string"
            ? t(error)
            : describeApiError(error, language)}
        </p>
      )}

      <FieldDescription>
        <Link
          href={docPath("subscribe")}
          target="_blank"
          rel="noreferrer"
          className="underline underline-offset-2"
        >
          {t("「外部カレンダーで見る」")}
        </Link>
        <SettingsHint label={t("外部カレンダーでの購読")}>
          {t(
            "Google カレンダーや Apple カレンダーなどに予定を表示できます。反映までの時間は各サービスの更新間隔によります。",
          )}
        </SettingsHint>
      </FieldDescription>

      <AlertDialog
        open={confirmation !== null}
        onOpenChange={(open) => {
          if (!open) setConfirmation(null);
        }}
      >
        <AlertDialogContent lang={language}>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {confirmation === "revoke"
                ? t("フィード URL を無効化しますか?")
                : t("フィード URL を再発行しますか?")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {confirmation === "revoke"
                ? t(
                    "今の URL は使えなくなり、購読しているカレンダーには予定が届かなくなります。もう一度使うには改めて発行します",
                  )
                : t(
                    "新しい URL に置き換わり、今の URL は使えなくなります。購読している人には新しい URL を登録し直してもらってください",
                  )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("キャンセル")}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={confirm}>
              {confirmation === "revoke" ? t("無効化") : t("再発行")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
