"use client";

import { DiscordSDK, patchUrlMappings } from "@discord/embedded-app-sdk";
import { useEffect, useRef, useState } from "react";
import { GuildDashboard } from "@/components/guild-dashboard";
import { GuildUnavailable } from "@/components/guild-unavailable";
import { useLanguage } from "@/components/language-provider";
import {
  ACTIVITY_SCOPES,
  externalLinkUrl,
  inviteUrlForGuild,
} from "@/lib/activity/links";
import { ApiError, api } from "@/lib/api";
import { setApiAuthToken } from "@/lib/api/client";
import type { Guild } from "@/lib/api/types";
import { SITE_URL } from "@/lib/site";

type State =
  | { status: "loading" }
  | { status: "outside-discord" }
  | { status: "failed" }
  | { status: "no-guild" }
  | { status: "unavailable"; guildId: string; error: unknown }
  | { status: "ready"; guild: Guild };

/**
 * Discord の Activity (#350) としてカレンダーを開く画面。
 *
 * Activity は `<client_id>.discordsays.com` (Discord のプロキシ) の iframe で動き、cookie のセッションが届かない。
 * そのため SDK の authorize → `/api/auth/discord-activity/sign-in` で署名付きセッショントークンを受け取り、
 * メモリに持って api へ `Authorization: Bearer` で送る (lib/api/client.ts の setApiAuthToken)。
 * 権限の判定は通常の Web と同じく api が行う
 */
export function ActivityApp({
  clientId,
  inviteUrl,
}: {
  clientId: string;
  /** Bot の招待 URL (サーバー未指定)。Bot がいないサーバーの案内に使う */
  inviteUrl: string;
}) {
  const [state, setState] = useState<State>({ status: "loading" });
  const sdkRef = useRef<DiscordSDK | null>(null);
  const started = useRef(false);

  useEffect(() => {
    // StrictMode の二重実行で authorize を 2 回呼ばない
    if (started.current) return;
    started.current = true;
    const params = new URLSearchParams(window.location.search);
    if (!params.has("frame_id") || !params.has("instance_id")) {
      setState({ status: "outside-discord" });
      return;
    }
    void (async () => {
      try {
        const sdk = new DiscordSDK(clientId);
        sdkRef.current = sdk;
        await sdk.ready();
        // Discord の CSP はプロキシ以外への通信を止めるので、アイコン画像を URL マッピング経由にする
        // (Developer Portal の URL Mappings に /discord-cdn → cdn.discordapp.com を登録しておく。docs/discord-activity.md)
        patchUrlMappings(
          [{ prefix: "/discord-cdn", target: "cdn.discordapp.com" }],
          { patchSrcAttributes: true },
        );
        const { code } = await sdk.commands.authorize({
          client_id: clientId,
          response_type: "code",
          state: "",
          prompt: "none",
          scope: [...ACTIVITY_SCOPES],
        });
        const res = await fetch("/api/auth/discord-activity/sign-in", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ code }),
        });
        if (!res.ok) {
          setState({ status: "failed" });
          return;
        }
        const { token } = (await res.json()) as { token: string };
        setApiAuthToken(token);

        const guildId = sdk.guildId;
        if (!guildId) {
          setState({ status: "no-guild" });
          return;
        }
        try {
          const guild = await api.guilds.get(guildId);
          setState({ status: "ready", guild });
        } catch (error) {
          setState({ status: "unavailable", guildId, error });
        }
      } catch {
        setState({ status: "failed" });
      }
    })();
  }, [clientId]);

  // 外へのリンク (新しいタブ・別オリジン・Activity 以外の画面) は iframe の中では開けないので、
  // SDK の openExternalLink で Discord の確認を挟んでブラウザに渡す。
  // 既存のリンクに手を入れずに済むよう、document の capture フェーズでまとめて拾う
  useEffect(() => {
    const onClick = (event: MouseEvent) => {
      const sdk = sdkRef.current;
      if (!sdk || event.defaultPrevented || event.button !== 0) return;
      const anchor =
        event.target instanceof Element
          ? event.target.closest<HTMLAnchorElement>("a[href]")
          : null;
      if (!anchor) return;
      const url = externalLinkUrl(
        anchor.getAttribute("href") ?? "",
        anchor.getAttribute("target"),
        window.location.href,
        SITE_URL,
      );
      if (!url) return;
      event.preventDefault();
      event.stopPropagation();
      void sdk.commands.openExternalLink({ url });
    };
    document.addEventListener("click", onClick, true);
    return () => document.removeEventListener("click", onClick, true);
  }, []);

  switch (state.status) {
    case "loading":
      return <Notice title="読み込み中…" />;
    case "outside-discord":
      return (
        <Notice
          title="この画面は Discord のアクティビティ専用です"
          body="ブラウザではダッシュボードからカレンダーを開いてください。"
        />
      );
    case "failed":
      return (
        <Notice
          title="ログインできませんでした"
          body="アクティビティを開き直してください。続く場合は、ブラウザから DisCalendar を開いてください。"
        />
      );
    case "no-guild":
      return (
        <Notice
          title="サーバーのチャンネルから起動してください"
          body="DM やグループ DM ではカレンダーを開けません。"
        />
      );
    case "unavailable":
      return (
        <GuildUnavailable
          error={state.error}
          action={
            state.error instanceof ApiError && state.error.status === 403 ? (
              <a
                href={inviteUrlForGuild(inviteUrl, state.guildId)}
                target="_blank"
                rel="noreferrer"
                className="rounded-full border border-foreground/20 px-5 py-2 text-sm hover:bg-foreground/10"
              >
                <InviteLabel />
              </a>
            ) : undefined
          }
        />
      );
    case "ready":
      return (
        <GuildDashboard
          guild={state.guild}
          guildChoices={[
            {
              id: state.guild.guild_id,
              name: state.guild.name,
              iconUrl: state.guild.avatar_url,
              accessLabel: null,
            },
          ]}
        />
      );
  }
}

function InviteLabel() {
  const { t } = useLanguage();
  return <>{t("BOT を導入する")}</>;
}

function Notice({ title, body }: { title: string; body?: string }) {
  const { t } = useLanguage();
  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-3 p-8 text-center">
      <h1 className="text-lg font-bold">{t(title)}</h1>
      {body && <p className="text-sm text-muted-foreground">{t(body)}</p>}
    </main>
  );
}
