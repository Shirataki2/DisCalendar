"use client";

import { KeyRoundIcon, WebhookIcon } from "lucide-react";
import {
  apiTokenAnchorId,
  formatJst,
  tokenStatus,
} from "@/components/api-token-settings";
import { useLanguage } from "@/components/language-provider";
import { webhookAnchorId } from "@/components/webhook-settings";
import { describeApiError } from "@/lib/api";
import type {
  GuildApiToken,
  GuildWebhook,
  MemberProfile,
} from "@/lib/api/types";
import { groupIntegrationsByCreator } from "@/lib/integrations-by-creator";
import {
  useGuildApiTokensQuery,
  useGuildWebhooksQuery,
  useMemberProfilesInChunks,
} from "@/lib/query/guild";

/**
 * 登録された Webhook と API トークンを、登録したメンバーごとにまとめた一覧。
 * 誰が何を外部につないでいるかを見渡せるようにする。操作は下の各節で行う (各行から飛べる)
 */
export function IntegrationsByCreator({ guildId }: { guildId: string }) {
  const { t, language } = useLanguage();
  const webhooksQuery = useGuildWebhooksQuery(guildId);
  const tokensQuery = useGuildApiTokensQuery(guildId);
  const groups = groupIntegrationsByCreator(
    webhooksQuery.data ?? [],
    tokensQuery.data ?? [],
  );
  const profiles = useMemberProfilesInChunks(
    guildId,
    groups.map((group) => group.userId).sort(),
    groups.length > 0,
  );

  const error = webhooksQuery.error ?? tokensQuery.error;
  return (
    <section
      aria-labelledby="integrations-by-creator-title"
      className="grid gap-3"
    >
      <h2
        id="integrations-by-creator-title"
        className="text-base font-semibold"
      >
        {t("登録したメンバーごとの一覧")}
      </h2>
      <p className="text-sm text-muted-foreground">
        {t(
          "このサーバーに登録された Webhook と API トークンを、登録したメンバーごとにまとめて表示します。",
        )}
      </p>

      {webhooksQuery.isPending || tokensQuery.isPending ? (
        <p className="text-sm">{t("読み込み中…")}</p>
      ) : error ? (
        <p role="alert" className="text-sm text-destructive">
          {describeApiError(error, language)}
        </p>
      ) : groups.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {t("登録された Webhook と API トークンはありません")}
        </p>
      ) : (
        <ul className="grid gap-3">
          {groups.map((group) => (
            <li key={group.userId} className="rounded-md border p-3">
              <CreatorName
                userId={group.userId}
                profiles={profiles.data}
                settled={!profiles.isPending}
              />
              <p className="mt-1 text-xs text-muted-foreground">
                {t("Webhook {webhooks} 件 · API トークン {tokens} 件", {
                  webhooks: group.webhooks.length,
                  tokens: group.tokens.length,
                })}
              </p>
              <ul className="mt-2 grid gap-1.5 text-sm">
                {group.webhooks.map((hook) => (
                  <WebhookRow key={`webhook-${hook.id}`} hook={hook} />
                ))}
                {group.tokens.map((token) => (
                  <TokenRow key={`token-${token.id}`} token={token} />
                ))}
              </ul>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function CreatorName({
  userId,
  profiles,
  settled,
}: {
  userId: string;
  profiles: MemberProfile[];
  settled: boolean;
}) {
  const { t } = useLanguage();
  const profile = profiles.find((member) => member.user_id === userId);
  if (!profile) {
    return (
      <p className="font-medium">
        {settled ? t("メンバー情報を取得できません") : t("読み込み中…")}
        <span className="ml-1 font-mono text-xs text-muted-foreground">
          ({userId})
        </span>
      </p>
    );
  }
  return (
    <p className="flex min-w-0 items-center gap-2 font-medium">
      {profile.avatar_url && (
        // biome-ignore lint/performance/noImgElement: Discord の小さなアバターを直接表示する
        <img
          src={profile.avatar_url}
          alt=""
          className="size-6 shrink-0 rounded-full"
        />
      )}
      <span className="min-w-0 break-words">
        {profile.display_name ?? t("退出したメンバー")}
      </span>
    </p>
  );
}

function WebhookRow({ hook }: { hook: GuildWebhook }) {
  const { t } = useLanguage();
  return (
    <li className="flex min-w-0 items-start gap-2">
      <WebhookIcon
        aria-hidden
        className="mt-0.5 size-4 shrink-0 text-muted-foreground"
      />
      <span className="min-w-0">
        <a
          href={`#${webhookAnchorId(hook.id)}`}
          className="break-all underline underline-offset-2"
        >
          {hook.url}
        </a>
        <span className="block text-xs text-muted-foreground">
          {"Webhook · "}
          {hook.kind === "json" ? "JSON" : "Discord"}
          {" · "}
          <span className={hook.enabled ? undefined : "text-destructive"}>
            {hook.enabled ? t("有効") : t("無効")}
          </span>
        </span>
      </span>
    </li>
  );
}

function TokenRow({ token }: { token: GuildApiToken }) {
  const { t, language } = useLanguage();
  const status = tokenStatus(token);
  return (
    <li className="flex min-w-0 items-start gap-2">
      <KeyRoundIcon
        aria-hidden
        className="mt-0.5 size-4 shrink-0 text-muted-foreground"
      />
      <span className="min-w-0">
        <a
          href={`#${apiTokenAnchorId(token.id)}`}
          className="break-all underline underline-offset-2"
        >
          {token.name}
        </a>{" "}
        <span className="font-mono text-xs text-muted-foreground">
          ({token.token_prefix}…)
        </span>
        <span className="block text-xs text-muted-foreground">
          {t("API トークン")}
          {" · "}
          {token.scope === "write" ? t("作成・更新・削除") : t("作成のみ")}
          {" · "}
          <span
            className={status === "active" ? undefined : "text-destructive"}
          >
            {status === "revoked"
              ? t("失効済み")
              : status === "expired"
                ? t("期限切れ")
                : t("有効")}
          </span>
          {" · "}
          {token.last_used_at
            ? t("最終利用: {date}", {
                date: formatJst(token.last_used_at, language),
              })
            : t("未使用")}
        </span>
      </span>
    </li>
  );
}
