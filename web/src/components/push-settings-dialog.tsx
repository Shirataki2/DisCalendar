"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useLanguage } from "@/components/language-provider";
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
  const { t, language } = useLanguage();
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
  const [name, setName] = useState<string | null>(null);
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
      <DialogContent lang={language} className="max-h-[85dvh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t("プッシュ通知")}</DialogTitle>
          <DialogDescription>
            {t(
              "予定の事前通知と開始時刻の通知を、登録した端末に届けます。通知範囲はすべての端末で共通です。",
            )}
          </DialogDescription>
        </DialogHeader>
        <p className="text-sm text-muted-foreground">
          {t(
            "iPhone / iPad では iOS 16.4 以降でホーム画面に追加してから有効にしてください。",
          )}
        </p>
        {query.isPending ? (
          <p role="status">{t("読み込み中…")}</p>
        ) : (
          query.data && (
            <>
              <fieldset disabled={action.isPending} className="space-y-2">
                <legend className="mb-2 text-sm font-medium">
                  {t("通知する予定")}
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
                    {t(label)}
                  </label>
                ))}
              </fieldset>
              <p className="text-xs text-muted-foreground">
                {t(
                  "作成者が記録されていない古い予定は「自分が作った予定だけ」の対象になりません。",
                )}
              </p>
              {!supported ? (
                <p role="status" className="text-sm">
                  {t("このブラウザではプッシュ通知に対応していません。")}
                </p>
              ) : !VAPID_PUBLIC_KEY ? (
                <p role="status" className="text-sm">
                  {t("プッシュ通知は現在準備中です。")}
                </p>
              ) : permission === "denied" ? (
                <p role="status" className="text-sm">
                  {t(
                    "通知が拒否されています。ブラウザのサイト設定で通知を許可してください。",
                  )}
                </p>
              ) : (
                <div className="space-y-2">
                  <label
                    htmlFor="push-device-name"
                    className="text-sm font-medium"
                  >
                    {t("端末名")}
                  </label>
                  <Input
                    id="push-device-name"
                    value={name ?? t("この端末")}
                    maxLength={80}
                    onChange={(e) => setName(e.target.value)}
                  />
                  <Button
                    disabled={
                      action.isPending ||
                      !(name ?? t("この端末")).trim() ||
                      query.data.scope === "off"
                    }
                    onClick={() => {
                      const registration = subscribeDevice(
                        (name ?? t("この端末")).trim(),
                        language,
                      );
                      action.mutate(() => registration);
                    }}
                  >
                    {t("この端末で受け取る")}
                  </Button>
                  {query.data.scope === "off" && (
                    <p className="text-xs text-muted-foreground">
                      {t("先に通知する予定を選んでください。")}
                    </p>
                  )}
                </div>
              )}
              <section aria-label={t("登録済みの端末")} className="space-y-2">
                <h3 className="text-sm font-medium">
                  {t("登録済みの端末 ({count}/10)", {
                    count: query.data.subscriptions.length,
                  })}
                </h3>
                {query.data.subscriptions.length === 0 && (
                  <p className="text-sm text-muted-foreground">
                    {t("登録された端末はありません。")}
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
                            ? t(
                                "送信に失敗したため停止中。端末で登録し直してください。",
                              )
                            : t("登録日: {date}", {
                                date: new Date(
                                  device.created_at,
                                ).toLocaleDateString(
                                  language === "en" ? "en-US" : "ja-JP",
                                  { timeZone: "Asia/Tokyo" },
                                ),
                              })}
                        </p>
                      </div>
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={action.isPending}
                        aria-label={t("{name}の登録を解除", {
                          name: device.device_name,
                        })}
                        onClick={() =>
                          action.mutate(() => api.push.remove(device.id))
                        }
                      >
                        {t("解除")}
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
              ? describeApiError(error, language)
              : t(error.message)}
          </p>
        )}
      </DialogContent>
    </Dialog>
  );
}
