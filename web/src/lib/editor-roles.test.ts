import { expect, test } from "vitest";
import type { MyPermissions } from "./api/types";
import {
  configToFormValues,
  formValuesToConfigInput,
  guildSettingsSchema,
} from "./guild-settings-form";
import { canEditEvents } from "./query/guild";

test("編集ロールは変更時だけ送り、空配列で解除する", () => {
  const values = configToFormValues({
    guild_id: "1",
    restricted: true,
    editor_role_ids: ["123"],
    notify_at_start: true,
    default_notifications: [],
    notification_channel_id: null,
    notification_channel_configured: false,
  });
  expect(values.editorRoleIds).toEqual(["123"]);
  expect(formValuesToConfigInput(values)).not.toHaveProperty("editor_role_ids");
  expect(formValuesToConfigInput(values, true).editor_role_ids).toEqual([
    "123",
  ]);
  expect(
    formValuesToConfigInput({ ...values, editorRoleIds: [] }, true)
      .editor_role_ids,
  ).toEqual([]);
  expect(
    guildSettingsSchema.safeParse({
      ...values,
      editorRoleIds: Array.from({ length: 26 }, (_, i) => String(i + 1)),
    }).success,
  ).toBe(false);
});

test("表示には API の編集判定だけを使い、取得前は許可しない", () => {
  expect(canEditEvents(undefined)).toBe(false);
  expect(
    canEditEvents({
      can_edit_events: true,
      can_manage_server: false,
    } as MyPermissions),
  ).toBe(true);
  expect(
    canEditEvents({
      can_edit_events: false,
      can_manage_server: true,
    } as MyPermissions),
  ).toBe(false);
});
