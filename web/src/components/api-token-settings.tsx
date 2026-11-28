"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckIcon, CopyIcon } from "lucide-react";
import Link from "next/link";
import { useId, useState } from "react";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api, describeApiError } from "@/lib/api";
import type { GuildApiToken, GuildApiTokenScope } from "@/lib/api/types";

const MAX_ACTIVE_TOKENS = 10;
const MAX_TOTAL_TOKENS = 20;
const NAME_MAX_CHARS = 64;

type ExpiryOption = "30" | "90" | "365" | "none";

function tokenStatus(token: GuildApiToken): "revoked" | "expired" | "active" {
  if (token.revoked_at) return "revoked";
  if (token.expires_at && new Date(token.expires_at).getTime() <= Date.now()) {
    return "expired";
  }
  return "active";
}

function formatJst(value: string, language: "ja" | "en"): string {
  return new Date(value).toLocaleString(language === "en" ? "en-US" : "ja-JP", {
    timeZone: "Asia/Tokyo",
  });
}

export function ApiTokenSettings({ guildId }: { guildId: string }) {
  const { t, language } = useLanguage();
  const id = useId();
  const client = useQueryClient();
  const key = ["guild", guildId, "api-tokens"];

  const [name, setName] = useState("");
  const [scope, setScope] = useState<GuildApiTokenScope>("create");
  const [expiry, setExpiry] = useState<ExpiryOption>("90");
  // 発行直後の平文トークンはコンポーネント内の state にだけ持ち、Query キャッシュへ残さない。
  const [issuedToken, setIssuedToken] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const query = useQuery({
    queryKey: key,
    queryFn: () => api.apiTokens.list(guildId),
    staleTime: 0,
  });

  const mutation = useMutation({
    mutationFn: async (action: () => Promise<void>) => {
      await action();
    },
    onSuccess: () => client.invalidateQueries({ queryKey: key }),
  });
  const run = (action: () => Promise<void>) => mutation.mutate(action);

  const tokens = query.data ?? [];
  const activeCount = tokens.filter((item) => !item.revoked_at).length;

  const copyIssuedToken = async () => {
    if (!issuedToken) return;
    await navigator.clipboard.writeText(issuedToken);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <section
      aria-labelledby={`${id}-title`}
      className="grid gap-3 border-t pt-4"
    >
      <h3 id={`${id}-title`} className="text-sm font-medium">
        {t("API トークン")}
      </h3>
      <p className="text-sm text-muted-foreground">
        {t(
          "外部のスクリプトや GitHub Actions などから HTTP 経由で予定を作成・更新・削除するためのトークンです。操作はその場で反映されます。",
        )}{" "}
        <Link href="/docs/api-tokens" className="underline">
          {t("使い方")}
        </Link>
      </p>

      {query.isPending && <p className="text-sm">{t("読み込み中…")}</p>}
      {(query.isError || mutation.isError) && (
        <p role="alert" className="text-sm text-destructive">
          {describeApiError(mutation.error ?? query.error, language)}
        </p>
      )}

      {issuedToken && (
        <div className="grid gap-2 rounded-md border p-3">
          <label htmlFor={`${id}-issued`} className="text-sm font-medium">
            {t("API トークン（一度だけ表示）")}
          </label>
          <p className="text-sm">
            {t(
              "安全な場所に保存してください。この画面を閉じると再表示できません。",
            )}
          </p>
          <div className="flex gap-2">
            <Input
              id={`${id}-issued`}
              value={issuedToken}
              readOnly
              autoComplete="off"
              className="font-mono text-xs"
            />
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void copyIssuedToken()}
            >
              {copied ? (
                <CheckIcon data-icon="inline-start" className="size-4" />
              ) : (
                <CopyIcon data-icon="inline-start" className="size-4" />
              )}
              {copied ? t("コピーしました") : t("コピー")}
            </Button>
          </div>
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              setIssuedToken(null);
              setCopied(false);
            }}
          >
            {t("保存したので閉じる")}
          </Button>
        </div>
      )}

      <fieldset disabled={mutation.isPending} className="grid gap-3">
        <div className="grid gap-2">
          <label htmlFor={`${id}-name`} className="text-sm">
            {t("トークン名")}
          </label>
          <Input
            id={`${id}-name`}
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("例: 配信スケジュール連携")}
            autoComplete="off"
            maxLength={NAME_MAX_CHARS}
          />
          <div className="grid gap-2 sm:grid-cols-2">
            <div className="grid gap-1">
              <label htmlFor={`${id}-scope`} className="text-sm">
                {t("権限")}
              </label>
              <select
                id={`${id}-scope`}
                value={scope}
                onChange={(e) =>
                  setScope(e.target.value === "write" ? "write" : "create")
                }
                className="h-9 rounded-md border bg-background px-3 text-sm"
              >
                <option value="create">{t("作成のみ")}</option>
                <option value="write">{t("作成・更新・削除")}</option>
              </select>
            </div>
            <div className="grid gap-1">
              <label htmlFor={`${id}-expiry`} className="text-sm">
                {t("有効期限")}
              </label>
              <select
                id={`${id}-expiry`}
                value={expiry}
                onChange={(e) => setExpiry(e.target.value as ExpiryOption)}
                className="h-9 rounded-md border bg-background px-3 text-sm"
              >
                <option value="30">{t("30 日")}</option>
                <option value="90">{t("90 日")}</option>
                <option value="365">{t("365 日")}</option>
                <option value="none">{t("無期限")}</option>
              </select>
            </div>
          </div>
          <Button
            type="button"
            disabled={
              !name.trim() ||
              !query.isSuccess ||
              activeCount >= MAX_ACTIVE_TOKENS ||
              tokens.length >= MAX_TOTAL_TOKENS ||
              !!issuedToken
            }
            onClick={() =>
              run(async () => {
                const result = await api.apiTokens.create(guildId, {
                  name: name.trim(),
                  scope,
                  expires_in_days: expiry === "none" ? null : Number(expiry),
                });
                setIssuedToken(result.token);
                setCopied(false);
                setName("");
              })
            }
          >
            {t("API トークンを発行（有効 {count}/10）", {
              count: activeCount,
            })}
          </Button>
        </div>

        {tokens.map((token) => {
          const status = tokenStatus(token);
          return (
            <div key={token.id} className="grid gap-2 rounded-md border p-3">
              <p className="break-all text-sm font-medium">
                {token.name}{" "}
                <span className="font-mono text-xs text-muted-foreground">
                  ({token.token_prefix}…)
                </span>
              </p>
              <p className="text-xs text-muted-foreground">
                {token.scope === "write"
                  ? t("作成・更新・削除")
                  : t("作成のみ")}
                {" · "}
                {status === "revoked"
                  ? t("失効済み")
                  : status === "expired"
                    ? t("期限切れ")
                    : t("有効")}
                {" · "}
                {token.expires_at
                  ? t("期限: {date}", {
                      date: formatJst(token.expires_at, language),
                    })
                  : t("無期限")}
                {" · "}
                {token.last_used_at
                  ? t("最終利用: {date}", {
                      date: formatJst(token.last_used_at, language),
                    })
                  : t("未使用")}
              </p>
              <div className="flex flex-wrap gap-2">
                {status !== "revoked" && (
                  <>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      disabled={!!issuedToken}
                      onClick={() => {
                        if (
                          window.confirm(
                            t(
                              "古いトークンは即座に使えなくなります。新しいトークンを再発行しますか？",
                            ),
                          )
                        ) {
                          run(async () => {
                            const result = await api.apiTokens.rotate(
                              guildId,
                              token.id,
                            );
                            setIssuedToken(result.token);
                            setCopied(false);
                          });
                        }
                      }}
                    >
                      {t("ローテーション")}
                    </Button>
                    <Button
                      type="button"
                      size="sm"
                      variant="outline"
                      onClick={() => {
                        if (
                          window.confirm(
                            t("この API トークンを失効させますか？"),
                          )
                        ) {
                          run(() => api.apiTokens.revoke(guildId, token.id));
                        }
                      }}
                    >
                      {t("失効させる")}
                    </Button>
                  </>
                )}
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  onClick={() => {
                    if (
                      window.confirm(
                        t("この API トークンと呼び出しログを削除しますか？"),
                      )
                    ) {
                      run(() => api.apiTokens.remove(guildId, token.id));
                    }
                  }}
                >
                  {t("削除")}
                </Button>
              </div>
              <details>
                <summary className="cursor-pointer text-sm">
                  {t("直近の呼び出しログ（最大20件）")}
                </summary>
                {token.logs.length === 0 ? (
                  <p className="mt-2 text-sm">
                    {t("呼び出し履歴はありません")}
                  </p>
                ) : (
                  <ul className="mt-2 grid max-h-48 gap-1.5 overflow-y-auto text-xs">
                    {token.logs.map((log) => (
                      <li key={log.id} className="break-all">
                        <time>{formatJst(log.called_at, language)}</time>
                        {" · "}
                        {log.method} {log.action}
                        {log.event_id !== null ? ` (#${log.event_id})` : ""}
                        {log.external_id ? ` [${log.external_id}]` : ""}
                        {" · "}
                        {log.status}
                        {log.error_kind ? ` (${log.error_kind})` : ""}
                      </li>
                    ))}
                  </ul>
                )}
              </details>
            </div>
          );
        })}
      </fieldset>
    </section>
  );
}
