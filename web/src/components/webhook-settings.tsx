"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { useId, useState } from "react";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api, describeApiError } from "@/lib/api";

export function WebhookSettings({ guildId }: { guildId: string }) {
  const { t, language } = useLanguage();
  const id = useId();
  const client = useQueryClient();
  const key = ["guild", guildId, "webhooks"];
  const [url, setUrl] = useState("");
  const [kind, setKind] = useState<"json" | "discord">("json");
  const [secret, setSecret] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const query = useQuery({
    queryKey: key,
    queryFn: () => api.webhooks.list(guildId),
    refetchInterval: 5000,
    staleTime: 0,
  });
  // シークレットを Query / Mutation キャッシュに保存しない。
  const mutation = useMutation({
    mutationFn: async (action: () => Promise<void>) => {
      setNotice(null);
      await action();
    },
    onSuccess: () => client.invalidateQueries({ queryKey: key }),
  });
  const run = (action: () => Promise<void>) => mutation.mutate(action);
  return (
    <section
      aria-labelledby={`${id}-title`}
      className="grid gap-3 border-t pt-4"
    >
      <h3 id={`${id}-title`} className="text-sm font-medium">
        Webhook
      </h3>
      <p className="text-sm text-muted-foreground">
        {t(
          "予定の作成・変更・削除を外部に通知します。説明を含む予定の内容が登録した URL に送られます。 操作はその場で反映されます。",
        )}{" "}
        <Link href="/docs/webhooks" className="underline">
          {t("使い方")}
        </Link>
      </p>
      {query.isPending && <p className="text-sm">{t("読み込み中…")}</p>}
      {(query.isError || mutation.isError) && (
        <p role="alert" className="text-sm text-destructive">
          {describeApiError(mutation.error ?? query.error, language)}
        </p>
      )}
      {notice && (
        <p role="status" className="text-sm">
          {t(notice)}
        </p>
      )}
      {secret && (
        <div className="grid gap-2 rounded-md border p-3">
          <label htmlFor={`${id}-secret`} className="text-sm font-medium">
            {t("署名用シークレット（一度だけ表示）")}
          </label>
          <p className="text-sm">
            {t(
              "安全な場所に保存してください。この画面を閉じると再表示できません。",
            )}
          </p>
          <Input
            id={`${id}-secret`}
            value={secret}
            readOnly
            autoComplete="off"
          />
          <Button
            type="button"
            variant="outline"
            onClick={() => setSecret(null)}
          >
            {t("保存したので閉じる")}
          </Button>
        </div>
      )}
      <fieldset disabled={mutation.isPending} className="grid gap-3">
        <div className="grid gap-2">
          <label htmlFor={`${id}-url`} className="text-sm">
            Webhook URL
          </label>
          <Input
            id={`${id}-url`}
            type="url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://example.com/webhook"
            autoComplete="off"
            maxLength={2048}
          />
          <label htmlFor={`${id}-kind`} className="text-sm">
            {t("種類")}
          </label>
          <select
            id={`${id}-kind`}
            value={kind}
            onChange={(e) =>
              setKind(e.target.value === "discord" ? "discord" : "json")
            }
            className="h-9 rounded-md border bg-background px-3 text-sm"
          >
            <option value="json">{t("汎用（JSON）")}</option>
            <option value="discord">{t("Discord Webhook 互換")}</option>
          </select>
          <Button
            type="button"
            disabled={
              !url.trim() ||
              !query.isSuccess ||
              query.data.length >= 5 ||
              !!secret
            }
            onClick={() =>
              run(async () => {
                const result = await api.webhooks.create(guildId, {
                  url: url.trim(),
                  kind,
                });
                setSecret(result.secret);
                setUrl("");
              })
            }
          >
            {t("Webhook を登録（{count}/5）", {
              count: query.data?.length ?? 0,
            })}
          </Button>
        </div>
        {query.data?.map((hook) => (
          <div key={hook.id} className="grid gap-2 rounded-md border p-3">
            <p className="break-all text-sm font-medium">
              #{hook.id} {hook.url} ·{" "}
              {hook.kind === "json" ? "JSON" : "Discord"}
            </p>
            <p className="text-sm">
              {hook.enabled ? t("有効") : t("無効")}
              {" · "}
              {t("連続失敗 {count} 回", { count: hook.consecutive_failures })}
            </p>
            {hook.disabled_reason && (
              <p className="text-sm text-destructive">
                {t(hook.disabled_reason)}
              </p>
            )}
            <div className="flex flex-wrap gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() =>
                  run(() =>
                    api.webhooks.setEnabled(guildId, hook.id, !hook.enabled),
                  )
                }
              >
                {hook.enabled ? t("無効にする") : t("有効にする")}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={!hook.enabled}
                onClick={() =>
                  run(async () => {
                    await api.webhooks.test(guildId, hook.id);
                    setNotice(
                      "テスト送信を予約しました。結果は配信ログに表示されます。",
                    );
                  })
                }
              >
                {t("テスト送信")}
              </Button>
              {hook.kind === "json" && (
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={!!secret}
                  onClick={() => {
                    if (
                      window.confirm(
                        t(
                          "古いシークレットは使えなくなります。再生成しますか？",
                        ),
                      )
                    )
                      run(async () => {
                        const result = await api.webhooks.rotate(
                          guildId,
                          hook.id,
                        );
                        setSecret(result.secret);
                      });
                  }}
                >
                  {t("シークレット再生成")}
                </Button>
              )}
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => {
                  if (
                    window.confirm(t("この Webhook と配信ログを削除しますか？"))
                  )
                    run(() => api.webhooks.remove(guildId, hook.id));
                }}
              >
                {t("削除")}
              </Button>
            </div>
            <details>
              <summary className="cursor-pointer text-sm">
                {t("直近の配信ログ（最大20件）")}
              </summary>
              {hook.deliveries.length === 0 ? (
                <p className="mt-2 text-sm">{t("配信履歴はありません")}</p>
              ) : (
                <ul className="mt-2 grid max-h-48 gap-2 overflow-y-auto text-xs">
                  {hook.deliveries.map((delivery) => (
                    <li
                      key={`${delivery.delivery_id}-${delivery.attempted_at}`}
                    >
                      <time>
                        {new Date(delivery.attempted_at).toLocaleString(
                          language === "en" ? "en-US" : "ja-JP",
                          { timeZone: "Asia/Tokyo" },
                        )}
                      </time>{" "}
                      · {delivery.kind} · {delivery.status ?? t("応答なし")}
                      {delivery.error && <p>{t(delivery.error)}</p>}
                    </li>
                  ))}
                </ul>
              )}
            </details>
          </div>
        ))}
      </fieldset>
    </section>
  );
}
