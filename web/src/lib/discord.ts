import { headers } from "next/headers";
import { auth } from "@/lib/auth";

const DISCORD_API = "https://discord.com/api/v10";

export interface DiscordGuild {
  id: string;
  name: string;
  icon: string | null;
  owner: boolean;
  permissions: string;
}

// Discord API の呼び出しはアクセストークンごとサーバー側で完結させる
// (旧実装はブラウザから直接呼んでいた)
export async function getUserGuilds(): Promise<DiscordGuild[]> {
  const requestHeaders = await headers();
  const accounts = await auth.api.listUserAccounts({
    headers: requestHeaders,
  });
  const discord = accounts.find((account) => account.providerId === "discord");
  if (!discord) {
    throw new Error("Discord account is not linked");
  }
  // 期限切れの場合は refresh token で自動更新される
  const { accessToken } = await auth.api.getAccessToken({
    body: { accountId: discord.id },
    headers: requestHeaders,
  });
  const res = await fetch(`${DISCORD_API}/users/@me/guilds`, {
    headers: { Authorization: `Bearer ${accessToken}` },
    next: { revalidate: 60 },
  });
  if (!res.ok) {
    throw new Error(`Discord API error: ${res.status}`);
  }
  return res.json();
}

export function guildIconUrl(guild: DiscordGuild): string | null {
  return guild.icon
    ? `https://cdn.discordapp.com/icons/${guild.id}/${guild.icon}.png?size=128`
    : null;
}
