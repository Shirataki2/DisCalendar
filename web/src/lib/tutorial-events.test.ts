import { expect, test, vi } from "vitest";
import { defaultEventFormValues, eventFormToApiInput } from "./event-form";
import {
  createTutorialEventsSource,
  TUTORIAL_GUILD_ID,
} from "./tutorial-events";

test("練習予定の作成・編集・削除と期間検索はその体験内だけに反映する", async () => {
  const changed = vi.fn();
  const now = new Date(2026, 8, 16, 10);
  const source = createTutorialEventsSource(changed, now);
  const other = createTutorialEventsSource(vi.fn(), now);
  const input = eventFormToApiInput({
    ...defaultEventFormValues(now),
    name: "練習の予定",
  });
  const list = () =>
    source.client.list(
      TUTORIAL_GUILD_ID,
      "2026-09-16T00:00:00",
      "2026-09-17T00:00:00",
    );
  const initial = await list();
  const event = await source.client.create(TUTORIAL_GUILD_ID, input);
  expect(await list()).toHaveLength(initial.length + 1);
  expect(
    await other.client.list(
      TUTORIAL_GUILD_ID,
      "2026-09-16T00:00:00",
      "2026-09-17T00:00:00",
    ),
  ).toEqual(initial);

  const updated = await source.client.update(TUTORIAL_GUILD_ID, event.id, {
    ...input,
    name: "月をまたぐ終日予定",
    is_all_day: true,
    start_at: "2026-09-30T00:00:00",
    end_at: "2026-10-02T00:00:00",
  });
  expect(await list()).toEqual(initial);
  expect(
    await source.client.list(
      TUTORIAL_GUILD_ID,
      "2026-10-02T00:00:00",
      "2026-10-03T00:00:00",
    ),
  ).toEqual([updated]);
  expect(
    await source.client.list(
      TUTORIAL_GUILD_ID,
      "2026-10-03T00:00:00",
      "2026-10-04T00:00:00",
    ),
  ).toEqual([]);
  await source.client.remove(TUTORIAL_GUILD_ID, event.id);
  expect(changed.mock.calls.map(([action]) => action)).toEqual([
    "create",
    "update",
    "remove",
  ]);
  expect(source.keys.all(TUTORIAL_GUILD_ID)[0]).toBe("tutorial");
  expect(source.afterCountChanged).toBeUndefined();
});

test("不正な入力や別サーバーへの操作では保存も案内の進行もしない", async () => {
  const changed = vi.fn();
  const source = createTutorialEventsSource(changed);
  const input = eventFormToApiInput(defaultEventFormValues());
  await expect(
    source.client.create(TUTORIAL_GUILD_ID, input),
  ).rejects.toThrow();
  await expect(
    source.client.create("real-guild", { ...input, name: "予定" }),
  ).rejects.toThrow();
  await expect(
    source.client.update(TUTORIAL_GUILD_ID, 999, { ...input, name: "予定" }),
  ).rejects.toThrow();
  expect(changed).not.toHaveBeenCalled();
});
