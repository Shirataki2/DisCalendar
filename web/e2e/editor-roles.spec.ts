import { expect, type Page, test } from "@playwright/test";
import { Pool } from "pg";
import { setEditorRoles } from "./discord-mock";
import { DATABASE_URL } from "./env";
import {
  E2E_BOT_ROLE_ID,
  E2E_EDITOR_ROLES,
  E2E_GUILDS,
  E2E_MANAGER_ROLE_ID,
  E2E_USER_ROLE_ID,
} from "./fixtures";

// 同じケースで複数回ロールを付け替えるため、再確認の制限時間を含める。
test.setTimeout(120_000);

const guild = E2E_GUILDS.editorRoles;
const role = E2E_EDITOR_ROLES[0];
const configUrl = `/local/api/guilds/${guild.id}/config`;
const rolesUrl = `/local/api/guilds/${guild.id}/roles`;
const permissionUrl = `/local/api/guilds/${guild.id}/@me/permissions`;
const eventsUrl = `/local/api/events/${guild.id}`;
const restrictedLabel =
  "予定の編集を管理権限または指定ロールのあるメンバーに限定する";
const event = {
  name: "E2E 編集ロールの予定",
  description: "",
  color: "#2196F3",
  notifications: [],
  is_all_day: false,
  start_at: "2026-10-01T10:00:00",
  end_at: "2026-10-01T11:00:00",
};

// 既存の 10 秒の再確認制限を保ったまま、前のテストのキャッシュが切り替わるまで待つ。
async function setAccess(
  page: Page,
  ids: string[],
  canManage: boolean,
  canEdit: boolean,
) {
  await setEditorRoles(guild.id, ids);
  await expect
    .poll(
      async () => {
        const response = await page.request.post(`${permissionUrl}/refresh`);
        expect(response.status()).toBe(200);
        const permissions = await response.json();
        return [permissions.can_manage_server, permissions.can_edit_events];
      },
      { timeout: 20_000, intervals: [1100] },
    )
    .toEqual([canManage, canEdit]);
}
async function openSettings(page: Page) {
  await page.getByRole("button", { name: "サーバー設定" }).click();
  const dialog = page.getByRole("dialog", { name: "サーバー設定" });
  await expect(
    dialog.getByRole("button", { name: "保存", exact: true }),
  ).toBeEnabled();
  return dialog;
}

test.beforeEach(async ({ page }) => {
  const pool = new Pool({ connectionString: DATABASE_URL });
  try {
    await pool.query(
      "INSERT INTO guild_config (guild_id, restricted, editor_role_ids) VALUES ($1, TRUE, '{}') ON CONFLICT (guild_id) DO UPDATE SET restricted=TRUE, editor_role_ids='{}'",
      [guild.id],
    );
    await pool.query("DELETE FROM events WHERE guild_id=$1", [guild.id]);
  } finally {
    await pool.end();
  }
  await setAccess(page, [E2E_MANAGER_ROLE_ID], true, true);
});

test("編集ロールによるCRUD・共有リンクをAPIが認可し、設定とフィード管理は拒否する", async ({
  page,
}) => {
  const selected = await page.request.put(configUrl, {
    data: { restricted: true, editor_role_ids: [role.id] },
  });
  expect(selected.status()).toBe(200);
  await setAccess(page, [role.id], false, true);
  const created = await page.request.post(eventsUrl, { data: event });
  expect(created.status()).toBe(201);
  const id = (await created.json()).id;
  expect(
    (
      await page.request.put(`${eventsUrl}/${id}`, {
        data: { ...event, name: "変更済み" },
      })
    ).status(),
  ).toBe(200);
  expect((await page.request.post(`${eventsUrl}/${id}/share`)).ok()).toBe(true);
  expect((await page.request.get(`${eventsUrl}/${id}/share`)).status()).toBe(
    200,
  );
  expect((await page.request.delete(`${eventsUrl}/${id}/share`)).status()).toBe(
    204,
  );
  expect(
    (
      await page.request.put(configUrl, { data: { restricted: false } })
    ).status(),
  ).toBe(403);
  expect(
    (await page.request.post(`/local/api/guilds/${guild.id}/feed`)).status(),
  ).toBe(403);
  expect(
    (await page.request.delete(`/local/api/guilds/${guild.id}/feed`)).status(),
  ).toBe(403);
  // 編集ロールだけでは Discord のイベント作成権限も増えない。
  expect((await page.request.get(permissionUrl)).ok()).toBe(true);
  const permissions = await (await page.request.get(permissionUrl)).json();
  expect(permissions.create_events).toBe(false);
  expect((await page.request.delete(`${eventsUrl}/${id}`)).status()).toBe(204);
  const remaining = await page.request.post(eventsUrl, { data: event });
  const remainingId = (await remaining.json()).id;
  await setAccess(page, [], false, false);
  expect((await page.request.post(eventsUrl, { data: event })).status()).toBe(
    403,
  );
  expect(
    (
      await page.request.put(`${eventsUrl}/${remainingId}`, { data: event })
    ).status(),
  ).toBe(403);
  expect(
    (await page.request.delete(`${eventsUrl}/${remainingId}`)).status(),
  ).toBe(403);
  for (const method of ["get", "post", "delete"] as const) {
    expect(
      (
        await page.request[method](`${eventsUrl}/${remainingId}/share`)
      ).status(),
    ).toBe(403);
  }
  await setAccess(page, [E2E_MANAGER_ROLE_ID], true, true);
  expect(
    (
      await page.request.put(configUrl, { data: { restricted: false } })
    ).status(),
  ).toBe(200);
  await setAccess(page, [], false, true);
  expect((await page.request.post(eventsUrl, { data: event })).status()).toBe(
    201,
  );
});

test("ロール一覧と設定APIの入力検証・部分更新", async ({ page }) => {
  const list = await (await page.request.get(rolesUrl)).json();
  expect(list.map((r: { id: string }) => r.id)).not.toContain(guild.id);
  expect(list.map((r: { id: string }) => r.id)).not.toContain(E2E_BOT_ROLE_ID);
  expect(list.find((r: { id: string }) => r.id === role.id)).toEqual({
    id: role.id,
    name: role.name,
    color: role.color,
    position: role.position,
    mentionable: false,
  });
  expect(list.map((r: { position: number }) => r.position)).toEqual(
    list
      .map((r: { position: number }) => r.position)
      .sort((a: number, b: number) => b - a),
  );
  for (const ids of [
    [guild.id],
    [E2E_BOT_ROLE_ID],
    [E2E_USER_ROLE_ID],
    ["999999999999999999"],
    ["abc"],
    [""],
    [role.id, role.id],
    E2E_EDITOR_ROLES.map((r) => r.id),
  ]) {
    expect(
      (
        await page.request.put(configUrl, {
          data: { restricted: true, editor_role_ids: ids },
        })
      ).status(),
    ).toBe(400);
  }
  const ids = E2E_EDITOR_ROLES.slice(0, 25).map((r) => r.id);
  expect(
    (
      await page.request.put(configUrl, {
        data: { restricted: true, editor_role_ids: ids },
      })
    ).status(),
  ).toBe(200);
  const partial = await page.request.put(configUrl, {
    data: { restricted: false, notify_at_start: false },
  });
  expect((await partial.json()).editor_role_ids).toEqual(ids);
  const cleared = await page.request.put(configUrl, {
    data: { restricted: true, editor_role_ids: [] },
  });
  expect((await cleared.json()).editor_role_ids).toEqual([]);
  expect(
    (
      await page.request.get(`/local/api/guilds/999999999999999999/roles`)
    ).status(),
  ).toBe(403);
});

test("チェック一覧を保存・再表示でき、OFFで保持し25件で追加だけ無効化する", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guild.id}`);
  let dialog = await openSettings(page);
  await dialog.getByRole("checkbox", { name: role.name, exact: true }).check();
  await dialog.getByRole("button", { name: "再読込", exact: true }).click();
  await expect(
    dialog.getByRole("button", { name: "再読込", exact: true }),
  ).toBeEnabled();
  await expect(
    dialog.getByRole("checkbox", { name: role.name, exact: true }),
  ).toBeChecked();
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  await expect(dialog).toBeHidden();
  dialog = await openSettings(page);
  const checkbox = dialog.getByRole("checkbox", {
    name: role.name,
    exact: true,
  });
  await expect(checkbox).toBeChecked();
  await dialog.getByRole("checkbox", { name: restrictedLabel }).uncheck();
  await expect(checkbox).toBeDisabled();
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  expect(
    (await (await page.request.get(configUrl)).json()).editor_role_ids,
  ).toEqual([role.id]);
  await page.request.put(configUrl, {
    data: {
      restricted: true,
      editor_role_ids: E2E_EDITOR_ROLES.slice(0, 25).map((r) => r.id),
    },
  });
  dialog = await openSettings(page);
  await expect(
    dialog.getByRole("checkbox", {
      name: E2E_EDITOR_ROLES[25].name,
      exact: true,
    }),
  ).toBeDisabled();
  await dialog
    .getByRole("checkbox", { name: role.name, exact: true })
    .uncheck();
  await expect(
    dialog.getByRole("checkbox", {
      name: E2E_EDITOR_ROLES[25].name,
      exact: true,
    }),
  ).toBeEnabled();
});

test("付与・剥奪後の再読込で画面が変わり、一般メンバーは設定を変更できない", async ({
  page,
}) => {
  await page.request.put(configUrl, {
    data: { restricted: true, editor_role_ids: [role.id] },
  });
  await setAccess(page, [], false, false);
  const roleRequests: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.endsWith(`/guilds/${guild.id}/roles`))
      roleRequests.push(request.url());
  });
  await page.goto(`/dashboard/${guild.id}`);
  await expect(page.getByRole("button", { name: "新規作成" })).toBeDisabled();
  await page.getByRole("button", { name: "サーバー設定" }).click();
  const dialog = page.getByRole("dialog", { name: "サーバー設定" });
  await expect(
    dialog.getByRole("button", { name: "保存", exact: true }),
  ).toBeDisabled();
  await expect(
    dialog.getByRole("checkbox", { name: role.name, exact: true }),
  ).toHaveCount(0);
  await expect(dialog.getByText(/編集を許可するロール/)).toHaveCount(0);
  for (const ids of [[role.id], []]) {
    await setEditorRoles(guild.id, ids);
    await expect(async () => {
      await dialog.getByRole("button", { name: "再読込", exact: true }).click();
      await expect(
        dialog.getByRole("button", { name: "再読込", exact: true }),
      ).toBeEnabled();
      // モーダル背後のボタンも同じクエリの更新を受ける。
      if (ids.length)
        await expect(
          page.getByRole("button", { name: "新規作成", includeHidden: true }),
        ).toBeEnabled({ timeout: 500 });
      else
        await expect(
          page.getByRole("button", { name: "新規作成", includeHidden: true }),
        ).toBeDisabled({ timeout: 500 });
    }).toPass({ timeout: 20_000, intervals: [1100] });
    await expect(dialog.getByText(/編集を許可するロール/)).toHaveCount(0);
  }
  expect(roleRequests).toEqual([]);
});

test("削除済みロールは解除でき、一覧取得失敗は削除と誤表示しない", async ({
  page,
}) => {
  await page.request.put(configUrl, {
    data: { restricted: true, editor_role_ids: [role.id] },
  });
  await setEditorRoles(guild.id, [E2E_MANAGER_ROLE_ID], [role.id]);
  await expect
    .poll(
      async () => {
        await page.request.post(`${permissionUrl}/refresh`);
        return (await (await page.request.get(rolesUrl)).json()).some(
          (r: { id: string }) => r.id === role.id,
        );
      },
      { timeout: 20_000, intervals: [1100] },
    )
    .toBe(false);
  expect(
    (
      await page.request.put(configUrl, {
        data: { restricted: true, editor_role_ids: [role.id] },
      })
    ).status(),
  ).toBe(400);
  await page.goto(`/dashboard/${guild.id}`);
  let dialog = await openSettings(page);
  await expect(
    dialog.getByText(`削除されたロール (ID: ${role.id})`, { exact: true }),
  ).toBeVisible();
  // 無関係な保存で削除済みロールを勝手に消さない。
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  expect(
    (await (await page.request.get(configUrl)).json()).editor_role_ids,
  ).toEqual([role.id]);
  dialog = await openSettings(page);
  await dialog
    .getByRole("checkbox", { name: E2E_EDITOR_ROLES[1].name, exact: true })
    .check();
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "削除されたロールを外して",
  );
  // 解除すると行ごと消えるため、uncheck の事後チェックではなく消失を確認する。
  const deletedRole = dialog.getByRole("checkbox", {
    name: `削除されたロール (ID: ${role.id})`,
  });
  await deletedRole.click();
  await expect(deletedRole).toBeHidden();
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  await expect(dialog).toBeHidden();
  await page.route(`**${rolesUrl}`, (route) =>
    route.fulfill({
      status: 502,
      contentType: "application/json",
      body: JSON.stringify({ error: "discord_error", message: "test" }),
    }),
  );
  dialog = await openSettings(page);
  await expect(dialog.getByRole("alert")).toContainText(
    "ロール一覧を取得できませんでした",
  );
  await expect(dialog.getByText(/削除されたロール/)).toHaveCount(0);
  await page.unroute(`**${rolesUrl}`);
  await dialog.getByRole("button", { name: "再読込", exact: true }).click();
  await expect(
    dialog.getByRole("checkbox", {
      name: E2E_EDITOR_ROLES[1].name,
      exact: true,
    }),
  ).toBeChecked();
});

test("設定保存後は権限を取り直し、取得失敗時に古い編集許可を使わない", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guild.id}`);
  await expect(page.getByRole("button", { name: "新規作成" })).toBeEnabled();
  const dialog = await openSettings(page);
  // DB 保存は実 API を通し、その後の権限取得だけを失敗させる。
  await page.route(`**${permissionUrl}`, (route) =>
    route.fulfill({
      status: 502,
      contentType: "application/json",
      body: JSON.stringify({ error: "discord_error", message: "test" }),
    }),
  );
  const refreshed = page.waitForResponse((response) =>
    response.url().endsWith(permissionUrl),
  );
  await dialog.getByRole("checkbox", { name: restrictedLabel }).uncheck();
  await dialog.getByRole("button", { name: "保存", exact: true }).click();
  expect((await refreshed).status()).toBe(502);
  await expect(dialog).toBeHidden();
  await expect(page.getByRole("button", { name: "新規作成" })).toBeDisabled();
  expect((await (await page.request.get(configUrl)).json()).restricted).toBe(
    false,
  );
});
