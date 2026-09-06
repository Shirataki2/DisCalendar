import { z } from "zod";
import type { GuildConfig, GuildConfigInput } from "@/lib/api/types";
import { NOTIFICATIONS_MAX, notificationSchema } from "@/lib/event-form";

// サーバー設定ダイアログ (guild-settings-dialog.tsx) のフォームと API との相互変換 (#181)。
// 通知の上限値は予定ダイアログ (event-form.ts) と同じ

export const guildSettingsSchema = z.object({
  restricted: z.boolean(),
  /** 予定の開始時刻に通知する */
  notifyAtStart: z.boolean(),
  /** 新しい予定の既定の事前通知。フィールド名は NotificationsField (共通部品) に合わせる */
  notifications: z
    .array(notificationSchema)
    .max(NOTIFICATIONS_MAX, `通知は${NOTIFICATIONS_MAX}件まで設定できます`),
  /** 通知先チャンネルの ID。"" は未設定 */
  notificationChannelId: z.string(),
});

export type GuildSettingsFormValues = z.infer<typeof guildSettingsSchema>;

export function configToFormValues(
  config: GuildConfig,
): GuildSettingsFormValues {
  return {
    restricted: config.restricted,
    notifyAtStart: config.notify_at_start,
    notifications: config.default_notifications.map(({ num, unit }) => ({
      num,
      unit,
    })),
    notificationChannelId: config.notification_channel_id ?? "",
  };
}

/** 未設定 ("") のままなら通知先は送らない (api 側で「変更しない」扱い) */
export function formValuesToConfigInput(
  values: GuildSettingsFormValues,
): GuildConfigInput {
  return {
    restricted: values.restricted,
    notify_at_start: values.notifyAtStart,
    default_notifications: values.notifications,
    ...(values.notificationChannelId
      ? { notification_channel_id: values.notificationChannelId }
      : {}),
  };
}
