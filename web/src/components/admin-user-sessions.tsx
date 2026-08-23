"use client";

import { Loader2Icon, LogOutIcon } from "lucide-react";
import { useState } from "react";
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
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { formatDateTime } from "@/lib/admin-format";
import { describeApiError } from "@/lib/api";
import {
  ADMIN_USER_SESSION_LIMIT,
  type AdminSession,
  type AdminUserSummary,
} from "@/lib/api/types";
import {
  useAdminUserSessions,
  useRevokeUserSessions,
} from "@/lib/query/admin-users";

/**
 * ユーザーのセッション一覧と強制ログアウト (#37)。
 * api はセッショントークンを返さないので、ここに出るのは作成・更新・期限と IP / User-Agent だけ。
 * 削除は api 側で監査ログ (`user.revoke_sessions`) に残る
 */
export function AdminUserSessionsButton({
  user,
  isSelf,
}: {
  user: AdminUserSummary;
  /** ログイン中の管理者自身か (自分を強制ログアウトすると自分も追い出される) */
  isSelf: boolean;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => setOpen(true)}
      >
        セッション
        <span className="ml-1 tabular-nums">
          {user.active_sessions.toLocaleString()}
        </span>
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>{user.name} のセッション</DialogTitle>
            <DialogDescription>
              Better Auth の session テーブルの内容 (トークンは表示しない)。
              新しい順に {ADMIN_USER_SESSION_LIMIT} 件まで
            </DialogDescription>
          </DialogHeader>
          {/* Base UI の Dialog は閉じると unmount されるので、開くたびに取り直される */}
          <SessionList user={user} isSelf={isSelf} />
        </DialogContent>
      </Dialog>
    </>
  );
}

function SessionList({
  user,
  isSelf,
}: {
  user: AdminUserSummary;
  isSelf: boolean;
}) {
  const sessions = useAdminUserSessions(user.id, true);
  const revoke = useRevokeUserSessions(user.id);
  const [confirming, setConfirming] = useState(false);
  const [message, setMessage] = useState<{
    kind: "ok" | "error";
    text: string;
  } | null>(null);

  const run = async () => {
    setConfirming(false);
    setMessage(null);
    try {
      const result = await revoke.mutateAsync();
      setMessage({
        kind: "ok",
        text: `${result.deleted.toLocaleString()} 件のセッションを削除しました`,
      });
    } catch (error) {
      setMessage({ kind: "error", text: describeApiError(error) });
    }
  };

  return (
    <div className="flex flex-col gap-3">
      {sessions.isPending ? (
        <p className="text-sm text-neutral-500">読み込み中…</p>
      ) : sessions.isError ? (
        <p role="alert" className="text-sm text-red-300">
          {describeApiError(sessions.error)}
        </p>
      ) : sessions.data.length === 0 ? (
        <p className="text-sm text-neutral-500">セッションはありません</p>
      ) : (
        <div className="max-h-80 overflow-auto rounded-lg border border-white/10">
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-neutral-900 text-left text-neutral-400">
              <tr>
                <th className="px-2 py-1.5 font-medium">ログイン</th>
                <th className="px-2 py-1.5 font-medium">有効期限</th>
                <th className="px-2 py-1.5 font-medium">IP</th>
                <th className="px-2 py-1.5 font-medium">User-Agent</th>
              </tr>
            </thead>
            <tbody>
              {sessions.data.map((session) => (
                <SessionRow key={session.id} session={session} />
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="flex flex-wrap items-center gap-3">
        <Button
          type="button"
          variant="destructive"
          size="sm"
          disabled={revoke.isPending || sessions.data?.length === 0}
          onClick={() => setConfirming(true)}
        >
          {revoke.isPending ? (
            <Loader2Icon data-icon="inline-start" className="animate-spin" />
          ) : (
            <LogOutIcon data-icon="inline-start" />
          )}
          強制ログアウト
        </Button>
        {isSelf && (
          <span className="text-xs text-amber-300">
            これはあなた自身のアカウントです
            (実行すると自分もログアウトされます)
          </span>
        )}
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
      </div>
      <AlertDialog open={confirming} onOpenChange={setConfirming}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>強制ログアウトしますか？</AlertDialogTitle>
            <AlertDialogDescription>
              {user.name} ({user.email}) のセッションをすべて削除します。
              このユーザーは次のリクエストからログインし直しになります。
              {isSelf && " これはあなた自身のアカウントです。"}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>キャンセル</AlertDialogCancel>
            <AlertDialogAction variant="destructive" onClick={() => void run()}>
              実行
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function SessionRow({ session }: { session: AdminSession }) {
  return (
    <tr className="border-t border-white/5">
      <td className="px-2 py-1.5 whitespace-nowrap">
        {formatDateTime(session.created_at)}
      </td>
      <td className="px-2 py-1.5 whitespace-nowrap">
        {formatDateTime(session.expires_at)}
        {session.expired && (
          <span className="ml-2 rounded bg-neutral-500/20 px-1.5 py-0.5 text-[10px] text-neutral-300">
            期限切れ
          </span>
        )}
      </td>
      <td className="px-2 py-1.5 font-mono">{session.ip_address ?? "-"}</td>
      <td
        className="max-w-64 truncate px-2 py-1.5 text-neutral-400"
        title={session.user_agent ?? undefined}
      >
        {session.user_agent ?? "-"}
      </td>
    </tr>
  );
}
