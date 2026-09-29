import { expect, test } from "@playwright/test";
import { Pool } from "pg";
import { setGuildEventPermissions } from "./discord-mock";
import { DATABASE_URL } from "./env";
import { E2E_CHANNELS, E2E_GUILDS } from "./fixtures";

const guild = E2E_GUILDS.polls.id;
test("日程調整を作成・投票・確定するとカレンダーに予定が出る", async ({
  page,
}) => {
  await page.goto(`/dashboard/${guild}/polls`);
  await page
    .getByRole("button", { name: "日程調整を作成", exact: true })
    .click();
  const title = `E2E 調整 ${Date.now().toString(36)}`;
  const form = page.getByRole("dialog", { name: "日程調整を作成" });
  await form.getByLabel("タイトル", { exact: true }).fill(title);
  await form.getByRole("button", { name: "候補を追加" }).click();
  await form
    .getByRole("button", { name: "日程調整を作成", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: title, exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "○ 参加できる", exact: true })
    .first()
    .click();
  await expect(
    page.getByRole("button", { name: "○ 参加できる", exact: true }).first(),
  ).toHaveAttribute("aria-pressed", "true");
  await page
    .getByRole("button", { name: "△ 未定", exact: true })
    .first()
    .click();
  await expect(
    page.getByRole("button", { name: "△ 未定", exact: true }).first(),
  ).toHaveAttribute("aria-pressed", "true");
  expect(new URL(page.url()).search).toBe("");
  await page.reload();
  await expect(
    page.getByRole("button", { name: "△ 未定", exact: true }).first(),
  ).toHaveAttribute("aria-pressed", "true");
  await page.screenshot({ path: "/tmp/169-poll-desktop.png" });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: "/tmp/169-poll-mobile.png" });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page
    .getByRole("button", { name: "この候補で確定", exact: true })
    .first()
    .click();
  const event = page.getByRole("dialog", { name: "予定を作成" });
  await expect(event.getByLabel("タイトル", { exact: false })).toHaveValue(
    title,
  );
  await expect(event.getByLabel("開始日", { exact: true })).toBeDisabled();
  const confirmed = page.waitForResponse(
    (r) => r.url().endsWith("/confirm") && r.request().method() === "POST",
  );
  await event.getByRole("button", { name: "作成", exact: true }).click();
  const response = await confirmed;
  expect(response.status()).toBe(201);
  const result = await response.json();
  expect(result.event.notifications.length).toBeGreaterThan(0);
  await expect(
    page.getByText("日程が確定しました。", { exact: false }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "この候補で確定" }),
  ).toHaveCount(0);
  const pollId = Number(new URL(page.url()).pathname.split("/").at(-1));
  const duplicate = await page.request.post(
    `/local/api/polls/${guild}/${pollId}/confirm`,
    {
      data: {
        option_id: 1,
        expected_version: 1,
        event: { ...result.event, discord_scheduled_event: false },
      },
    },
  );
  expect(duplicate.status()).toBe(409);
  await page.goto(
    `/dashboard/${guild}?date=${result.event.start_at.slice(0, 10)}`,
  );
  await expect(page.getByText(title, { exact: true }).first()).toBeVisible();
  await page.request.delete(`/local/api/polls/${guild}/${pollId}`);
  await page.request.delete(`/local/api/events/${guild}/${result.event.id}`);
});

test("権限のないメンバーは作成不可、非メンバーは閲覧不可", async ({ page }) => {
  const input = {
    title: "権限確認",
    options: [
      {
        start_at: "2099-10-01T20:00:00",
        end_at: "2099-10-01T21:00:00",
        is_all_day: false,
      },
    ],
  };
  expect(
    (
      await page.request.post(`/local/api/polls/${E2E_GUILDS.member.id}`, {
        data: input,
      })
    ).status(),
  ).toBe(403);
  expect(
    (await page.request.get("/local/api/polls/999999999999999999")).status(),
  ).toBe(403);
});

test("一般メンバーの投票・締切とDiscord連携付き確定を検証する", async ({
  page,
}) => {
  const db = new Pool({ connectionString: DATABASE_URL });
  let pollId: number | undefined;
  let eventId: number | undefined;
  const isolatedGuild = E2E_GUILDS.noUserEventsPerm.id;
  try {
    const option = {
      start_at: "2099-10-01T20:00:00",
      end_at: "2099-10-01T21:00:00",
      is_all_day: false,
    };
    // restrictedでも投票は許可する。テスト用の日程調整だけ直接準備する。
    const inserted = await db.query(
      "INSERT INTO schedule_polls(guild_id,title,created_by,created_at) VALUES ($1,'一般メンバーの投票','123',now()) RETURNING id",
      [E2E_GUILDS.member.id],
    );
    const memberPoll = inserted.rows[0].id;
    const insertedOption = await db.query(
      "INSERT INTO schedule_poll_options(poll_id,start_at,end_at,is_all_day,position) VALUES ($1,$2,$3,false,0) RETURNING id",
      [memberPoll, option.start_at, option.end_at],
    );
    const voteUrl = `/local/api/polls/${E2E_GUILDS.member.id}/${memberPoll}/vote`;
    const vote = { option_id: insertedOption.rows[0].id, answer: "yes" };
    expect((await page.request.put(voteUrl, { data: vote })).status()).toBe(
      204,
    );
    await db.query(
      "UPDATE schedule_polls SET deadline='2000-01-01' WHERE id=$1",
      [memberPoll],
    );
    expect((await page.request.put(voteUrl, { data: vote })).status()).toBe(
      409,
    );
    await db.query("DELETE FROM schedule_polls WHERE id=$1", [memberPoll]);
    // 利用者がDiscordイベント作成権限を持たない場合、予定と確定状態を残さない。
    const created = await page.request.post(
      `/local/api/polls/${isolatedGuild}`,
      { data: { title: "Discord権限", options: [option] } },
    );
    expect(created.status()).toBe(201);
    const { poll } = await created.json();
    pollId = poll.id;
    const event = {
      name: poll.title,
      ...option,
      color: "#123456",
      notifications: [{ num: 30, unit: "minutes" }],
      discord_scheduled_event: true,
    };
    const forbidden = await page.request.post(
      `/local/api/polls/${isolatedGuild}/${poll.id}/confirm`,
      {
        data: {
          option_id: poll.options[0].id,
          expected_version: poll.version,
          event,
        },
      },
    );
    expect(forbidden.status()).toBe(403);
    expect(
      (
        await (
          await page.request.get(`/local/api/polls/${isolatedGuild}/${poll.id}`)
        ).json()
      ).status,
    ).toBe("open");
    await page.request.delete(`/local/api/polls/${isolatedGuild}/${poll.id}`);
    pollId = undefined;
    // 管理者の確認は予定とDiscordの対応付けをまとめて保存する。
    const linked = await page.request.post(`/local/api/polls/${guild}`, {
      data: { title: "Discord連携付き確定", options: [option] },
    });
    const linkedPoll = (await linked.json()).poll;
    pollId = linkedPoll.id;
    await db.query(
      "INSERT INTO event_settings (guild_id,channel_id) VALUES ($1,$2)",
      [guild, E2E_CHANNELS.general.id],
    );
    const result = await page.request.post(
      `/local/api/polls/${guild}/${pollId}/confirm`,
      {
        data: {
          option_id: linkedPoll.options[0].id,
          expected_version: linkedPoll.version,
          event,
        },
      },
    );
    expect(result.status()).toBe(201);
    const body = await result.json();
    eventId = body.event.id;
    expect(body.event.discord_scheduled_event_id).toBeTruthy();
    expect(body.event.notifications).toEqual(event.notifications);
    expect(body.announcement).toBe("sent");
  } finally {
    if (pollId)
      await page.request.delete(`/local/api/polls/${guild}/${pollId}`);
    if (eventId)
      await page.request.delete(`/local/api/events/${guild}/${eventId}`);
    await db.query("DELETE FROM event_settings WHERE guild_id=$1", [guild]);
    await db.end();
  }
});

test("編集を開いた後の再取得でも他の編集者の変更を上書きしない", async ({
  page,
}) => {
  const input = {
    title: "競合確認",
    description: null,
    deadline: null,
    options: [
      {
        start_at: "2099-01-01T10:00:00",
        end_at: "2099-01-01T11:00:00",
        is_all_day: false,
      },
    ],
  };
  const created = await page.request.post(`/local/api/polls/${guild}`, {
    data: input,
  });
  const { poll } = await created.json();
  try {
    await page.goto(`/dashboard/${guild}/polls/${poll.id}`);
    await page.getByRole("button", { name: "候補を編集" }).click();
    const dialog = page.getByRole("dialog", { name: "日程調整を編集" });
    await dialog.getByLabel("タイトル", { exact: true }).fill("古い画面の変更");
    expect(
      (
        await page.request.put(`/local/api/polls/${guild}/${poll.id}`, {
          data: {
            ...input,
            title: "別の編集者の変更",
            options: poll.options,
            expected_version: poll.version,
          },
        })
      ).status(),
    ).toBe(200);
    // 詳細の15秒ポーリングがフォームへ新しいpropsを渡した後に保存する。
    await expect(
      page.getByRole("heading", {
        name: "別の編集者の変更",
        includeHidden: true,
      }),
    ).toBeVisible({ timeout: 25_000 });
    const response = page.waitForResponse(
      (r) =>
        r.url().endsWith(`/polls/${guild}/${poll.id}`) &&
        r.request().method() === "PUT",
    );
    await dialog.getByRole("button", { name: "変更を保存" }).click();
    expect((await response).status()).toBe(409);
    await expect(dialog.getByRole("alert")).toBeVisible();
    const current = await page.request.get(
      `/local/api/polls/${guild}/${poll.id}`,
    );
    expect((await current.json()).title).toBe("別の編集者の変更");
  } finally {
    await page.request.delete(`/local/api/polls/${guild}/${poll.id}`);
  }
});

test("確定中にBotの権限が失われたら再確認の導線を表示する", async ({
  page,
}) => {
  await setGuildEventPermissions(guild, {
    botCreateEvents: true,
    userCreateEvents: true,
  });
  await page.request.post(`/local/api/guilds/${guild}/@me/permissions/refresh`);
  const response = await page.request.post(`/local/api/polls/${guild}`, {
    data: {
      title: "Bot権限の再確認",
      options: [
        {
          start_at: "2099-01-01T10:00:00",
          end_at: "2099-01-01T11:00:00",
          is_all_day: false,
        },
      ],
    },
  });
  const { poll } = await response.json();
  try {
    await page.goto(`/dashboard/${guild}/polls/${poll.id}`);
    await page.getByRole("button", { name: "この候補で確定" }).click();
    const dialog = page.getByRole("dialog", { name: "予定を作成" });
    const checkbox = dialog.getByRole("checkbox", {
      name: "Discord のイベントとしても作成する",
    });
    await checkbox.check();
    await setGuildEventPermissions(guild, {
      botCreateEvents: false,
      userCreateEvents: true,
    });
    await dialog.getByRole("button", { name: "作成", exact: true }).click();
    await expect(checkbox).toBeDisabled();
    await expect(
      dialog.getByRole("button", { name: "権限を再確認" }),
    ).toBeVisible();
    expect(
      (
        await (
          await page.request.get(`/local/api/polls/${guild}/${poll.id}`)
        ).json()
      ).status,
    ).toBe("open");
  } finally {
    await setGuildEventPermissions(guild, null);
    await page.request.post(
      `/local/api/guilds/${guild}/@me/permissions/refresh`,
    );
    await page.request.delete(`/local/api/polls/${guild}/${poll.id}`);
  }
});

test.describe("JST候補と個人設定の引継ぎ", () => {
  test.use({ timezoneId: "America/New_York" });
  test("DSTで欠落する時刻もそのまま確定し、個人の色と通知なしを使う", async ({
    page,
  }) => {
    const option = {
      start_at: "2026-03-08T02:30:00",
      end_at: "2026-03-08T03:00:00",
      is_all_day: false,
    };
    const created = await page.request.post(`/local/api/polls/${guild}`, {
      data: { title: "DST候補", options: [option] },
    });
    expect(created.status()).toBe(201);
    const { poll } = await created.json();
    let eventId: number | undefined;
    try {
      await page.addInitScript(() =>
        localStorage.setItem(
          "discalendar-calendar-settings",
          JSON.stringify({ defaultColor: "#2196F3", defaultNotifications: [] }),
        ),
      );
      await page.goto(`/dashboard/${guild}/polls/${poll.id}`);
      await page
        .getByRole("button", { name: "この候補で確定", exact: true })
        .click();
      const dialog = page.getByRole("dialog", { name: "予定を作成" });
      await expect(dialog.getByLabel("開始時刻", { exact: true })).toHaveValue(
        "02:30",
      );
      const confirmed = page.waitForResponse(
        (r) => r.url().endsWith("/confirm") && r.request().method() === "POST",
      );
      await dialog.getByRole("button", { name: "作成", exact: true }).click();
      const response = await confirmed;
      expect(response.status()).toBe(201);
      const { event } = await response.json();
      eventId = event.id;
      expect(event).toMatchObject({
        ...option,
        color: "#2196F3",
        notifications: [],
      });
    } finally {
      await page.request.delete(`/local/api/polls/${guild}/${poll.id}`);
      if (eventId)
        await page.request.delete(`/local/api/events/${guild}/${eventId}`);
    }
  });
});
