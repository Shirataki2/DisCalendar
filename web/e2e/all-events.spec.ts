import { expect, test } from "@playwright/test";
import { calendarToday, eventOn, isoDate, openEventPopover } from "./calendar";
import { DATABASE_URL } from "./env";
import { E2E_GUILDS } from "./fixtures";
import { deleteEventsNamed, insertEvent } from "./seed";

// 参加している全サーバーの予定をまとめて見る「すべての予定」(#98)。
// 管理権限のあるギルド (admin) と、restricted で一般メンバーのギルド (member) の予定が
// 1 つのカレンダーに出て、凡例で絞り込め、ポップオーバーからそのサーバーへ移動できる。閲覧のみで作成はできない

test.describe.configure({ mode: "serial" });

// 他のテストと同じ DB を使うので、実行ごとに違う名前にして最後に消す
// (list-view.spec.ts は admin ギルドの当月に予定が無いことを前提にしている)
const stamp = Date.now().toString(36);
const adminTitle = `E2E 横断 ${stamp} admin`;
const memberTitle = `E2E 横断 ${stamp} member`;

test.afterAll(async () => {
  await deleteEventsNamed(DATABASE_URL, [adminTitle, memberTitle]);
});

test("両方のサーバーの予定がまとめて表示され、作成はできない", async ({
  page,
}) => {
  // 予定を仕込む。admin ギルドは API で、member ギルドは restricted で API が 403 を返すので DB に直接入れる
  await page.goto(`/dashboard/${E2E_GUILDS.admin.id}`);
  const today = isoDate(await calendarToday(page));
  const created = await page.request.post(
    `/local/api/events/${E2E_GUILDS.admin.id}`,
    {
      data: {
        name: adminTitle,
        description: null,
        notifications: [],
        color: "#F44336",
        is_all_day: false,
        start_at: `${today}T10:00:00`,
        end_at: `${today}T11:00:00`,
        discord_scheduled_event: false,
      },
    },
  );
  expect(created.status(), await created.text()).toBe(201);
  await insertEvent(DATABASE_URL, E2E_GUILDS.member.id, {
    name: memberTitle,
    start_at: `${today}T13:00:00`,
    end_at: `${today}T14:00:00`,
  });

  await page.goto("/dashboard/all");
  await expect(
    page.getByRole("heading", { name: "すべての予定" }),
  ).toBeVisible();
  await expect(page.getByRole("grid")).toBeVisible();
  await expect(eventOn(page, adminTitle)).toBeVisible();
  await expect(eventOn(page, memberTitle)).toBeVisible();
  await expect(page.getByRole("button", { name: "新規作成" })).toHaveCount(0);

  // 凡例に両方のサーバーが出ている
  const legend = page.getByRole("list", { name: "サーバーの凡例" });
  await expect(
    legend.getByRole("button", { name: E2E_GUILDS.admin.name }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    legend.getByRole("button", { name: E2E_GUILDS.member.name }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("凡例のチップでサーバーごとに表示を絞り込める", async ({ page }) => {
  await page.goto("/dashboard/all");
  await expect(eventOn(page, memberTitle)).toBeVisible();

  const chip = page
    .getByRole("list", { name: "サーバーの凡例" })
    .getByRole("button", { name: E2E_GUILDS.member.name });
  await chip.click();
  await expect(chip).toHaveAttribute("aria-pressed", "false");
  await expect(eventOn(page, memberTitle)).toHaveCount(0);
  await expect(eventOn(page, adminTitle)).toBeVisible();

  await chip.click();
  await expect(chip).toHaveAttribute("aria-pressed", "true");
  await expect(eventOn(page, memberTitle)).toBeVisible();
});

test("予定のポップオーバーにサーバー名が出て、そのサーバーのカレンダーへ移動できる", async ({
  page,
}) => {
  await page.goto("/dashboard/all");
  const popover = await openEventPopover(page, adminTitle);
  await expect(popover).toContainText(E2E_GUILDS.admin.name);
  // 閲覧のみ (編集の操作は無い)
  await expect(popover.getByRole("button", { name: "編集" })).toHaveCount(0);
  await expect(popover.getByRole("button", { name: "削除" })).toHaveCount(0);

  const link = popover.getByRole("link", {
    name: "このサーバーのカレンダーを開く",
  });
  await expect(link).toHaveAttribute(
    "href",
    `/dashboard/${E2E_GUILDS.admin.id}`,
  );
  await link.click();
  await expect(page).toHaveURL(`/dashboard/${E2E_GUILDS.admin.id}`);
  await expect(
    page.getByRole("main").getByText(E2E_GUILDS.admin.name),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "新規作成" })).toBeEnabled();
});

test("サイドバーとサーバー選択画面から開ける", async ({ page }) => {
  await page.goto("/dashboard/all");
  const nav = page.getByRole("navigation", { name: "サイト内メニュー" });
  await expect(nav.getByRole("link", { name: "すべての予定" })).toHaveAttribute(
    "aria-current",
    "page",
  );
  // 「サーバー一覧」の配下の URL だが、こちらは現在地にしない
  await expect(
    nav.getByRole("link", { name: "サーバー一覧" }),
  ).not.toHaveAttribute("aria-current", "page");

  // サーバー選択画面のカード (サイドバーにも同名のリンクがあるので本文に絞る)
  await page.goto("/dashboard");
  await expect(
    page.getByRole("main").getByRole("link", { name: "すべての予定" }),
  ).toHaveAttribute("href", "/dashboard/all");
});
