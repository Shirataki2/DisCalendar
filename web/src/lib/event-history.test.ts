import { describe, expect, test } from "vitest";
import type { EventHistoryEntry, EventSnapshot } from "@/lib/api/types";
import {
  diffEventSnapshots,
  formatHistoryTime,
  historyActorIds,
} from "./event-history";

const base: EventSnapshot = {
  name: "定例",
  description: null,
  location: null,
  color: "#2196F3",
  is_all_day: false,
  start_at: "2026-09-05T21:00:00",
  end_at: "2026-09-05T22:00:00",
  notifications: [{ num: 10, unit: "minutes" }],
  notification_mentions: [],
  discord_linked: false,
};

function entry(
  id: number,
  actor: string | null,
  source: EventHistoryEntry["source"] = "web",
): EventHistoryEntry {
  return {
    id,
    event_id: 1,
    actor_discord_user_id: actor,
    source,
    action: "update",
    before: base,
    after: base,
    created_at: "2026-09-03T03:00:00Z",
  };
}

describe("diffEventSnapshots", () => {
  test("変わっていなければ何も返さない", () => {
    expect(diffEventSnapshots(base, { ...base })).toEqual([]);
  });

  test("変わった項目だけを予定ダイアログの並びで返す", () => {
    const after: EventSnapshot = {
      ...base,
      name: "定例 (延期)",
      start_at: "2026-09-06T21:00:00",
      end_at: "2026-09-06T22:00:00",
      notifications: [],
      discord_linked: true,
    };
    expect(diffEventSnapshots(base, after).map((c) => c.field)).toEqual([
      "name",
      "schedule",
      "notifications",
      "discord",
    ]);
    expect(diffEventSnapshots(base, after)[0]).toEqual({
      field: "name",
      before: "定例",
      after: "定例 (延期)",
    });
  });

  test("終日への切り替えは日時の変更として扱い、色の大文字小文字は区別しない", () => {
    const after = { ...base, is_all_day: true, color: "#2196f3" };
    expect(diffEventSnapshots(base, after).map((c) => c.field)).toEqual([
      "schedule",
    ]);
  });

  test("説明・場所・メンション先の変更を拾う", () => {
    const after: EventSnapshot = {
      ...base,
      description: "持ち物あり",
      location: "会議室 A",
      notification_mentions: [{ type: "everyone" }],
    };
    expect(diffEventSnapshots(base, after)).toEqual([
      { field: "location", before: null, after: "会議室 A" },
      { field: "description" },
      { field: "mentions" },
    ]);
  });
});

describe("historyActorIds", () => {
  test("重複と管理コンソールの操作者を除き、20 件までに絞る", () => {
    const entries = [
      entry(3, "111"),
      entry(2, "222", "admin"),
      entry(1, "111", "bot"),
      entry(0, null),
      ...Array.from({ length: 30 }, (_, i) => entry(-i - 1, `9${i}`)),
    ];
    const ids = historyActorIds(entries);
    expect(ids).toHaveLength(20);
    expect(ids[0]).toBe("111");
    expect(ids).not.toContain("222");
  });
});

describe("formatHistoryTime", () => {
  test("UTC の時刻を JST で表す", () => {
    expect(formatHistoryTime("2026-09-03T03:00:00Z")).toBe("2026/9/3 12:00");
    expect(formatHistoryTime("2026-12-31T15:30:00+00:00")).toBe(
      "2027/1/1 00:30",
    );
  });
});
