"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ApiError, api, describeApiError, type PushScope } from "@/lib/api";
import { authClient } from "@/lib/auth-client";
import {
  PUSH_QUERY_KEY,
  pushSupported,
  subscribeDevice,
  VAPID_PUBLIC_KEY,
} from "@/lib/push";

export function PushSettingsDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const client = useQueryClient();
  const { data: session } = authClient.useSession();
  const query = useQuery({
    queryKey: [...PUSH_QUERY_KEY, session?.user.id],
    queryFn: api.push.get,
    enabled: open && !!session?.user.id,
  });
  const [supported, setSupported] = useState(false);
  const [permission, setPermission] =
    useState<NotificationPermission>("default");
  const [name, setName] = useState("この端末");
  useEffect(() => {
    if (!open) return;
    setSupported(pushSupported());
    if ("Notification" in window) setPermission(Notification.permission);
  }, [open]);
  const action = useMutation({
    mutationFn: (run: () => Promise<void>) => run(),
    onSettled: async () => {
      if ("Notification" in window) setPermission(Notification.permission);
      await client.invalidateQueries({ queryKey: PUSH_QUERY_KEY });
    },
  });
  const error = action.error ?? query.error;
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[85dvh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>プッシュ通知</DialogTitle>
          <DialogDescription>
            予定の事前通知と開始時刻の通知を、登録した端末に届けます。通知範囲はすべての端末で共通です。
          </DialogDescription>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">
          iPhone / iPad では iOS 16.4
          以降でホーム画面に追加してから有効にしてください。
        </p>
        {query.isPending ? (
          <p role="status">読み込み中…</p>
        ) : (
          query.data && (
            <>
              <fieldset disabled={action.isPending} className="space-y-2">
                <legend className="mb-2 text-sm font-medium">
                  通知する予定
                </legend>
                {(
                  [
                    ["all", "参加している全サーバーの予定"],
                    ["created", "自分が作った予定だけ"],
                    ["off", "オフ"],
                  ] as const
                ).map(([value, label]) => (
                  <label
                    key={value}
                    className="flex items-center gap-2 text-sm"
                  >
                    <input
                      type="radio"
                      name="push-scope"
                      value={value}
                      checked={query.data.scope === value}
                      onChange={() =>
                        action.mutate(() =>
                          api.push.setScope(value as PushScope),
                        )
                      }
                    />
                    {label}
                  </label>
                ))}
              </fieldset>
              <p className="text-xs text-muted-foreground">
                作成者が記録されていない古い予定は「自分が作った予定だけ」の対象になりません。
              </p>
              {!supported ? (
                <p role="status" className="text-sm">
                  このブラウザではプッシュ通知に対応していません。
                </p>
              ) : !VAPID_PUBLIC_KEY ? (
                <p role="status" className="text-sm">
                  プッシュ通知は現在準備中です。
                </p>
              ) : permission === "denied" ? (
                <p role="status" className="text-sm">
                  通知が拒否されています。ブラウザのサイト設定で通知を許可してください。
                </p>
              ) : (
                <div className="space-y-2">
                  <label
                    htmlFor="push-device-name"
                    className="text-sm font-medium"
                  >
                    端末名
                  </label>
                  <Input
                    id="push-device-name"
                    value={name}
                    maxLength={80}
                    onChange={(e) => setName(e.target.value)}
                  />
                  <Button
                    disabled={
                      action.isPending ||
                      !name.trim() ||
                      query.data.scope === "off"
                    }
                    onClick={() => {
                      const registration = subscribeDevice(name.trim());
                      action.mutate(() => registration);
                    }}
                  >
                    この端末で受け取る
                  </Button>
                  {query.data.scope === "off" && (
                    <p className="text-xs text-muted-foreground">
                      先に通知する予定を選んでください。
                    </p>
                  )}
                </div>
              )}
              <section aria-label="登録済みの端末" className="space-y-2">
                <h3 className="text-sm font-medium">
                  登録済みの端末 ({query.data.subscriptions.length}/10)
                </h3>
                {query.data.subscriptions.length === 0 && (
                  <p className="text-sm text-muted-foreground">
                    登録された端末はありません。
                  </p>
                )}
                <ul className="space-y-2">
                  {query.data.subscriptions.map((device) => (
                    <li
                      key={device.id}
                      className="flex items-center gap-2 rounded-md border p-2"
                    >
                      <div className="min-w-0 flex-1">
                        <p className="break-words text-sm">
                          {device.device_name}
                        </p>
                        <p className="text-xs text-muted-foreground">
                          {device.disabled
                            ? "送信に失敗したため停止中。端末で登録し直してください。"
                            : `登録日: ${new Date(device.created_at).toLocaleDateString("ja-JP")}`}
                        </p>
                      </div>
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={action.isPending}
                        aria-label={`${device.device_name}の登録を解除`}
                        onClick={() =>
                          action.mutate(() => api.push.remove(device.id))
                        }
                      >
                        解除
                      </Button>
                    </li>
                  ))}
                </ul>
              </section>
            </>
          )
        )}
        {error && (
          <p role="alert" className="text-sm text-destructive">
            {error instanceof ApiError
              ? describeApiError(error)
              : error.message}
          </p>
        )}
      </DialogContent>
    </Dialog>
  );
}
