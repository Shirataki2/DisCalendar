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

/**
 * サーバー設定の節の URL。`date` (カレンダーで表示していた日付、YYYY-MM-DD) を付けると、
 * 節を切り替えても持ち回り、「カレンダーに戻る」でその日付を開き直せる
 */
export function guildSettingsPath(
  guildId: string,
  section: GuildSettingsSection = "",
  date?: string,
): string {
  const query = date ? `?${new URLSearchParams({ date })}` : "";
  return `/dashboard/${guildId}/settings${section ? `/${section}` : ""}${query}`;
}

/** サーバー設定からカレンダーに戻る URL。表示していた日付があればその日付を開く */
export function guildCalendarPath(guildId: string, date?: string): string {
  const query = date ? `?${new URLSearchParams({ date })}` : "";
  return `/dashboard/${guildId}${query}`;
}
