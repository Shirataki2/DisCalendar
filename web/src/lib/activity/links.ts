import { SITE_URL } from "@/lib/site";

/** Activity の画面のパス。これ以外の画面は cookie のセッションが前提なので Activity の中では開かない */
export const ACTIVITY_PATH = "/activity";

/** Activity の authorize で求める scope。通常ログイン (lib/auth.ts) の identify / email / guilds と揃える */
export const ACTIVITY_SCOPES = ["identify", "email", "guilds"] as const;

/** Discord の Activity のプロキシ (`<client_id>.discordsays.com`) の中で動いているか */
export function isDiscordActivityHost(): boolean {
  return (
    typeof window !== "undefined" &&
    window.location.hostname.endsWith(".discordsays.com")
  );
}

/**
 * 利用者に渡す URL (共有リンクなど) の基準オリジン。Activity の中では表示中のオリジンが
 * Discord のプロキシなので、公開 URL に置き換える
 */
export function publicOrigin(): string {
  return isDiscordActivityHost()
    ? new URL(SITE_URL).origin
    : window.location.origin;
}

/** Bot の招待 URL をそのサーバーを選んだ状態にする (lib/discord.ts の botInviteUrl と同じ指定) */
export function inviteUrlForGuild(base: string, guildId: string): string {
  const url = new URL(base);
  url.searchParams.set("guild_id", guildId);
  url.searchParams.set("disable_guild_select", "true");
  return url.toString();
}

/**
 * Activity の中でリンクを押したときに、Discord の外 (ブラウザ) で開くべき URL を返す。
 * 開かなくてよい (Activity の中のページ内リンクなど) なら null。
 *
 * - 新しいタブ指定 (`target="_blank"`) や別オリジンのリンクはそのまま外で開く
 * - 同じオリジンでも Activity 以外の画面 (`/docs`、`/dashboard/...` など) は外で開く
 * - 同じオリジンの URL は Discord のプロキシ (discordsays.com) ではなく公開 URL (`siteUrl`) に直す
 */
export function externalLinkUrl(
  href: string,
  target: string | null,
  currentUrl: string,
  siteUrl: string,
): string | null {
  if (!href || href.startsWith("#")) {
    return null;
  }
  let url: URL;
  try {
    url = new URL(href, currentUrl);
  } catch {
    return null;
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    return null;
  }
  const current = new URL(currentUrl);
  if (url.origin !== current.origin) {
    return url.toString();
  }
  const insideActivity =
    url.pathname === ACTIVITY_PATH ||
    url.pathname.startsWith(`${ACTIVITY_PATH}/`);
  if (insideActivity && target !== "_blank") {
    return null;
  }
  return new URL(`${url.pathname}${url.search}${url.hash}`, siteUrl).toString();
}
