import { headers } from "next/headers";
import { auth } from "@/lib/auth";

// Discord API のベース URL。E2E テスト (e2e/) ではモックサーバーに向ける (api 側の DISCORD_API_BASE_URL と同じ扱い)
const DISCORD_API =
  process.env.DISCORD_API_BASE_URL ?? "https://discord.com/api/v10";

export interface DiscordGuild {
  id: string;
  name: string;
  icon: string | null;
  owner: boolean;
  /** ギルドでの権限ビット (文字列)。64bit を超えるので BigInt で扱う */
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

const ADMINISTRATOR = BigInt(1) << BigInt(3);
const MANAGE_GUILD = BigInt(1) << BigInt(5);

/** Bot をサーバーに追加できるか (「管理者」か「サーバー管理」。旧実装の `permissions & 40` と同じ) */
export function canInviteBot(guild: DiscordGuild): boolean {
  let permissions: bigint;
  try {
    permissions = BigInt(guild.permissions);
  } catch {
    return false;
  }
  return (permissions & (ADMINISTRATOR | MANAGE_GUILD)) !== BigInt(0);
}

// Bot に必要な権限: チャンネルを見る / メッセージを送信 / 埋め込みリンク /
// メッセージ履歴を読む / アプリコマンドを使う / イベントの管理 (#94)。
// bot/src/commands/invite.rs の required_bot_permissions() と揃える (向こうのテストが一致を確認する)
const DEFAULT_BOT_PERMISSIONS = "10737503232";

/**
 * Bot の招待 URL。DISCORD_BOT_INVITE_URL (旧実装の INVITATION_URL 相当) があればそれを使い、
 * なければ DISCORD_CLIENT_ID から組み立てる。guildId を渡すとそのサーバーを選んだ状態で開く
 * (サーバー選択画面)。渡さなければ Discord 側でサーバーを選ぶ (LP の「BOT を導入する」)
 */
export function botInviteUrl(guildId?: string): string {
  const base = process.env.DISCORD_BOT_INVITE_URL
    ? new URL(process.env.DISCORD_BOT_INVITE_URL)
    : new URL("https://discord.com/oauth2/authorize");
  if (!process.env.DISCORD_BOT_INVITE_URL) {
    base.searchParams.set("client_id", process.env.DISCORD_CLIENT_ID ?? "");
    base.searchParams.set("scope", "bot applications.commands");
    base.searchParams.set("permissions", DEFAULT_BOT_PERMISSIONS);
  }
  if (guildId) {
    base.searchParams.set("guild_id", guildId);
    base.searchParams.set("disable_guild_select", "true");
  }
  return base.toString();
}
