"use client";

import { useId, useState } from "react";
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
import { useLastValue } from "@/hooks/use-last-value";
import { describeApiError } from "@/lib/api";
import type { TrashedEvent } from "@/lib/api/types";
import { describeEventRange } from "@/lib/calendar-events";
import {
  usePurgeEvent,
  useRestoreEvent,
  useTrashQuery,
} from "@/lib/query/events";
import { useMemberProfilesInChunks } from "@/lib/query/guild";

/** API の JST naive をブラウザのタイムゾーンによらず表示する (event-authors.tsx と同じ形) */
function formatJst(value: string) {
  const [date, time] = value.split("T");
  const [, month, day] = date.split("-");
  return `${Number(month)}/${Number(day)} ${time.slice(0, 5)}`;
}

/**
 * サーバー設定の「削除した予定」(ゴミ箱、#159)。管理権限のある人だけに出す。
 * 削除から 30 日間は元に戻せ、過ぎると自動で完全に消える。操作はその場で反映される
 */
export function TrashSettings({ guildId }: { guildId: string }) {
  const { t, language } = useLanguage();
  const id = useId();
  const trash = useTrashQuery(guildId, true);
  const restore = useRestoreEvent(guildId);
  const purge = usePurgeEvent(guildId);
  const [purgeTarget, setPurgeTarget] = useState<TrashedEvent | null>(null);
  // 確認ダイアログを閉じるアニメーションの間も名前を出しておく
  const purgeShown = useLastValue(purgeTarget);
  const [notice, setNotice] = useState<string | null>(null);
  const events = trash.data?.pages.flatMap((page) => page.events) ?? [];
  const profiles = useMemberProfilesInChunks(
    guildId,
    events.flatMap((event) => (event.deleted_by ? [event.deleted_by] : [])),
    events.length > 0,
  );
  const busy = restore.isPending || purge.isPending;
  const error = restore.error ?? purge.error ?? trash.error;

  const deletedBy = (userId: string | null) => {
    if (!userId) return t("記録なし");
    const profile = profiles.data.find((member) => member.user_id === userId);
    if (profile) return profile.display_name ?? t("退出したメンバー");
    return profiles.isError
      ? t("メンバー情報を取得できません")
      : t("読み込み中…");
  };

  const start = () => {
    setNotice(null);
    restore.reset();
    purge.reset();
  };
  const restoreEvent = (event: TrashedEvent) => {
    start();
    restore.mutate(event.id, {
      onSuccess: () =>
        setNotice(t("「{name}」を元に戻しました。", { name: event.name })),
    });
  };
  const purgeEvent = (event: TrashedEvent) => {
    start();
    purge.mutate(event.id, {
      onSuccess: () =>
        setNotice(t("「{name}」を完全に削除しました。", { name: event.name })),
    });
  };

  return (
    <section
      aria-labelledby={`${id}-title`}
      className="grid gap-3 border-t pt-4"
    >
      <h3 id={`${id}-title`} className="text-sm font-medium">
        {t("削除した予定")}
      </h3>
      <p className="text-sm text-muted-foreground">
        {t(
          "削除した予定は 30 日間ここに残り、元に戻せます。30 日を過ぎると自動で完全に削除されます。操作はその場で反映されます。",
        )}
      </p>
      {trash.isPending && <p className="text-sm">{t("読み込み中…")}</p>}
      {!!error && (
        <p role="alert" className="text-sm text-destructive">
          {describeApiError(error, language)}
        </p>
      )}
      {notice && (
        <p role="status" className="text-sm">
          {notice}
        </p>
      )}
      {trash.isSuccess && events.length === 0 && (
        <p className="text-sm text-muted-foreground">
          {t("削除した予定はありません。")}
        </p>
      )}
      {events.length > 0 && (
        <ul className="grid max-h-80 gap-2 overflow-y-auto">
          {events.map((event) => (
            <li
              key={event.id}
              className="flex flex-wrap items-center justify-between gap-2 rounded-md border p-3"
            >
              <div className="grid min-w-0 gap-0.5 text-sm">
                <span className="break-words font-medium">{event.name}</span>
                <span className="text-muted-foreground">
                  {describeEventRange(event, language)}
                </span>
                <span className="text-muted-foreground">
                  {t("削除: {user} ({at}) · {until} まで元に戻せます", {
                    user: deletedBy(event.deleted_by),
                    at: formatJst(event.deleted_at),
                    until: formatJst(event.expires_at),
                  })}
                </span>
              </div>
              <div className="flex shrink-0 gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={busy}
                  aria-label={t("「{name}」を元に戻す", { name: event.name })}
                  onClick={() => restoreEvent(event)}
                >
                  {t("元に戻す")}
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="destructive"
                  disabled={busy}
                  aria-label={t("「{name}」を完全に削除", {
                    name: event.name,
                  })}
                  onClick={() => setPurgeTarget(event)}
                >
                  {t("完全に削除")}
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {trash.hasNextPage && (
        <Button
          type="button"
          size="sm"
          variant="outline"
          className="justify-self-start"
          disabled={trash.isFetchingNextPage}
          onClick={() => trash.fetchNextPage()}
        >
          {trash.isFetchingNextPage ? t("読み込み中…") : t("さらに表示")}
        </Button>
      )}
      <AlertDialog
        open={purgeTarget !== null}
        onOpenChange={(open) => {
          if (!open) setPurgeTarget(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("完全に削除しますか？")}</AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                "「{name}」を完全に削除します。添付ファイル・共有リンク・変更履歴も消え、元に戻せなくなります。",
                { name: purgeShown?.name ?? "" },
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("キャンセル")}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                if (!purgeTarget) return;
                const target = purgeTarget;
                setPurgeTarget(null);
                purgeEvent(target);
              }}
            >
              {t("完全に削除")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
