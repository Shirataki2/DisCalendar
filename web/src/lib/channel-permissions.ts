import type { GuildChannel, NotificationPermission } from "@/lib/api/types";

// 通知先チャンネル (#181) の表示用ヘルパ。権限の表示名は Bot の `/init` の返信 (bot/src/checks.rs の
// describe_permissions) と同じ言葉にする

const PERMISSION_LABELS: Record<NotificationPermission, string> = {
  view_channel: "チャンネルを見る",
  send_messages: "メッセージを送信",
  embed_links: "埋め込みリンク",
};

/** 足りない権限を「チャンネルを見る」「埋め込みリンク」のように並べる */
export function describeMissingPermissions(
  permissions: readonly NotificationPermission[],
): string {
  return permissions.map((p) => `「${PERMISSION_LABELS[p]}」`).join("");
}

/** Discord 風の表示名 (`#general`) */
export function channelLabel(channel: Pick<GuildChannel, "name">): string {
  return `#${channel.name}`;
}
