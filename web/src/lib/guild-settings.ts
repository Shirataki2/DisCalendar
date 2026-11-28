/**
 * サーバー設定ページの節。`slug` は `/dashboard/[id]/settings/<slug>` (空文字は `/settings` そのもの)。
 * `label` は日本語原文 (表示時に `t()` で訳す)
 */
export const GUILD_SETTINGS_SECTIONS = [
  { slug: "", label: "全般" },
  { slug: "calendars", label: "外部カレンダー" },
  { slug: "integrations", label: "Webhook・API トークン" },
  { slug: "trash", label: "削除した予定" },
] as const;

export type GuildSettingsSection =
  (typeof GUILD_SETTINGS_SECTIONS)[number]["slug"];

export function guildSettingsPath(
  guildId: string,
  section: GuildSettingsSection = "",
): string {
  return `/dashboard/${guildId}/settings${section ? `/${section}` : ""}`;
}
