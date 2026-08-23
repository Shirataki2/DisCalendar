"use client";

import { Loader2Icon, Trash2Icon } from "lucide-react";
import { type ReactNode, useId, useState } from "react";
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
import { Input } from "@/components/ui/input";
import { describeApiError } from "@/lib/api";
import { ADMIN_DELETE_SNAPSHOT_LIMIT } from "@/lib/api/types";
import {
  useDeleteGuildEvents,
  usePurgeExpiredSessions,
} from "@/lib/query/admin-sql";

// 管理コンソールの定型操作 (#36)。書き込みは自由 SQL ではなくここから行い、api 側で監査ログに残る。
// どの操作も確認ダイアログを挟む

/** 確認ダイアログ付きの実行ボタン。`run` の結果 (件数) と失敗をボタンの横に出す */
function ConfirmedOperation({
  label,
  title,
  description,
  disabled,
  run,
  format,
}: {
  label: string;
  title: string;
  description: ReactNode;
  disabled?: boolean;
  run: () => Promise<{ deleted: number }>;
  /** 成功時のメッセージ */
  format: (deleted: number) => string;
}) {
  const [open, setOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const [message, setMessage] = useState<{
    kind: "ok" | "error";
    text: string;
  } | null>(null);

  const confirm = async () => {
    setOpen(false);
    setPending(true);
    setMessage(null);
    try {
      const result = await run();
      setMessage({ kind: "ok", text: format(result.deleted) });
    } catch (error) {
      setMessage({ kind: "error", text: describeApiError(error) });
    } finally {
      setPending(false);
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-3">
      <Button
        type="button"
        variant="destructive"
        size="sm"
        disabled={disabled || pending}
        onClick={() => setOpen(true)}
      >
        {pending ? (
          <Loader2Icon data-icon="inline-start" className="animate-spin" />
        ) : (
          <Trash2Icon data-icon="inline-start" />
        )}
        {label}
      </Button>
      {message && (
        <output
          className={
            message.kind === "ok"
              ? "text-xs text-emerald-300"
              : "rounded-md bg-red-900/40 px-2 py-1 text-xs text-red-200"
          }
        >
          {message.text}
        </output>
      )}
      <AlertDialog open={open} onOpenChange={setOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{title}</AlertDialogTitle>
            <AlertDialogDescription>{description}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>キャンセル</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={confirm}>
              実行
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

/** 指定ギルドの予定をすべて削除するボタン (ギルド詳細と定型操作の一覧で使う) */
export function DeleteGuildEventsButton({
  guildId,
  guildName,
  eventCount,
}: {
  guildId: string;
  guildName?: string | null;
  /** 分かっていれば確認文に出す */
  eventCount?: number;
}) {
  const deleteEvents = useDeleteGuildEvents();
  const shown = guildName ? `${guildName} (${guildId})` : guildId;
  return (
    <ConfirmedOperation
      label="全予定を削除"
      title="このギルドの予定をすべて削除しますか？"
      description={
        <>
          {shown} の予定
          {eventCount !== undefined && ` ${eventCount.toLocaleString()} 件`}
          をすべて削除します。監査ログに残るのは先頭{" "}
          {ADMIN_DELETE_SNAPSHOT_LIMIT}{" "}
          件の内容と件数だけで、それ以降の内容は保存されません。
          この操作は取り消せません。
        </>
      }
      disabled={!/^\d{1,20}$/.test(guildId)}
      run={() => deleteEvents.mutateAsync(guildId)}
      format={(deleted) => `${deleted.toLocaleString()} 件の予定を削除しました`}
    />
  );
}

/** 定型操作の一覧 (`/admin/sql` の下部) */
export function AdminOps() {
  const [guildId, setGuildId] = useState("");
  const purge = usePurgeExpiredSessions();
  const guildInputId = useId();
  const trimmed = guildId.trim();

  return (
    <section aria-labelledby="ops-heading" className="flex flex-col gap-4">
      <div>
        <h2 id="ops-heading" className="text-base font-semibold">
          定型操作
        </h2>
        <p className="mt-1 text-xs text-neutral-400">
          書き込みを伴う操作はここから行う (自由 SQL での書き込みは不可)。
          どれも確認ダイアログの後に実行し、監査ログに残る。予定 1
          件の編集・削除と restricted の切替はギルド詳細から
        </p>
      </div>
      <div className="rounded-lg border border-white/10 p-4">
        <h3 className="text-sm font-medium">指定ギルドの全予定を削除</h3>
        <p className="mt-1 mb-3 text-xs text-neutral-400">
          退出済みギルドのデータ整理や、荒らされた予定の一括削除に。削除した予定は先頭{" "}
          {ADMIN_DELETE_SNAPSHOT_LIMIT} 件まで監査ログの before に残る
          (それ以降は件数のみ)
        </p>
        <div className="flex flex-wrap items-center gap-3">
          <label htmlFor={guildInputId} className="sr-only">
            ギルド ID
          </label>
          <Input
            id={guildInputId}
            value={guildId}
            onChange={(event) => setGuildId(event.target.value)}
            placeholder="ギルド ID"
            inputMode="numeric"
            className="w-56 font-mono"
          />
          <DeleteGuildEventsButton guildId={trimmed} />
        </div>
      </div>
      <div className="rounded-lg border border-white/10 p-4">
        <h3 className="text-sm font-medium">期限切れセッションの削除</h3>
        <p className="mt-1 mb-3 text-xs text-neutral-400">
          Better Auth の session のうち expiresAt を過ぎた行を消す
          (有効なセッションには触れないので、誰もログアウトされない)
        </p>
        <ConfirmedOperation
          label="期限切れセッションを削除"
          title="期限切れセッションを削除しますか？"
          description="session テーブルの期限切れの行をすべて削除します。"
          run={() => purge.mutateAsync()}
          format={(deleted) =>
            `${deleted.toLocaleString()} 件のセッションを削除しました`
          }
        />
      </div>
    </section>
  );
}
