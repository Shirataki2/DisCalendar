import { describe, expect, it } from "vitest";
import type { GuildConfig } from "@/lib/api/types";
import {
  configToFormValues,
  formValuesToConfigInput,
  guildSettingsSchema,
} from "./guild-settings-form";

const config: GuildConfig = {
  guild_id: "200000000000000001",
  restricted: true,
  notify_at_start: false,
  default_notifications: [{ num: 30, unit: "minutes" }],
  notification_channel_id: "400000000000000001",
  notification_channel_configured: true,
};

describe("configToFormValues / formValuesToConfigInput", () => {
  it("API の設定とフォームの値を往復できる", () => {
    const values = configToFormValues(config);
    expect(values).toEqual({
      restricted: true,
      notifyAtStart: false,
      notifications: [{ num: 30, unit: "minutes" }],
      notificationChannelId: "400000000000000001",
    });
    expect(formValuesToConfigInput(values)).toEqual({
      restricted: true,
      notify_at_start: false,
      default_notifications: [{ num: 30, unit: "minutes" }],
      notification_channel_id: "400000000000000001",
    });
  });

  it("通知先が未設定なら空文字で持ち、送信では省略する (api が「変更しない」と解釈する)", () => {
    const values = configToFormValues({
      ...config,
      notification_channel_id: null,
      notification_channel_configured: false,
    });
    expect(values.notificationChannelId).toBe("");
    const input = formValuesToConfigInput(values);
    expect("notification_channel_id" in input).toBe(false);
  });

  it("既定の事前通知は予定ダイアログと同じ検証 (件数と値域)", () => {
    const base = configToFormValues(config);
    expect(guildSettingsSchema.safeParse(base).success).toBe(true);
    expect(
      guildSettingsSchema.safeParse({
        ...base,
        notifications: [{ num: 0, unit: "minutes" }],
      }).success,
    ).toBe(false);
    expect(
      guildSettingsSchema.safeParse({
        ...base,
        notifications: Array.from({ length: 11 }, () => ({
          num: 1,
          unit: "hours",
        })),
      }).success,
    ).toBe(false);
  });
});
