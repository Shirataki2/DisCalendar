import type { EventHistoryEntry, EventSnapshot } from "@/lib/api/types";

/**
 * 変更履歴 (#165) の 1 件で変わった項目。表示の言葉は履歴ダイアログ側で付ける。
 * 日時は終日かどうかと開始・終了をまとめて「日時」として扱う (予定ダイアログの「日時」と同じ単位)
 */
export type EventHistoryChange =
  | { field: "name"; before: string; after: string }
  | {
      field: "schedule";
      before: Pick<EventSnapshot, "start_at" | "end_at" | "is_all_day">;
      after: Pick<EventSnapshot, "start_at" | "end_at" | "is_all_day">;
    }
  | { field: "location"; before: string | null; after: string | null }
  | { field: "description" }
  | { field: "recurrence" }
  | { field: "color"; before: string; after: string }
  | {
      field: "notifications";
      before: EventSnapshot["notifications"];
      after: EventSnapshot["notifications"];
    }
  | { field: "mentions" }
  | { field: "discord"; after: boolean };

/** 古い行や手で書き換えた行で項目が欠けていても比べられるよう、比較用に値を揃える */
function same(a: unknown, b: unknown): boolean {
  return JSON.stringify(a ?? null) === JSON.stringify(b ?? null);
}

/** 更新前後のスナップショットから、変わった項目だけを予定ダイアログの並びで返す */
export function diffEventSnapshots(
  before: EventSnapshot,
  after: EventSnapshot,
): EventHistoryChange[] {
  const changes: EventHistoryChange[] = [];
  if (before.name !== after.name) {
    changes.push({ field: "name", before: before.name, after: after.name });
  }
  if (
    before.start_at !== after.start_at ||
    before.end_at !== after.end_at ||
    before.is_all_day !== after.is_all_day
  ) {
    changes.push({ field: "schedule", before, after });
  }
  if (!same(before.location, after.location)) {
    changes.push({
      field: "location",
      before: before.location ?? null,
      after: after.location ?? null,
    });
  }
  if (after.description_changed) {
    changes.push({ field: "description" });
  }
  if (before.color.toLowerCase() !== after.color.toLowerCase()) {
    changes.push({ field: "color", before: before.color, after: after.color });
  }
  if (!same(before.notifications, after.notifications)) {
    changes.push({
      field: "notifications",
      before: before.notifications ?? [],
      after: after.notifications ?? [],
    });
  }
  if (!same(before.notification_mentions, after.notification_mentions)) {
    changes.push({ field: "mentions" });
  }
  if (!same(before.recurrence, after.recurrence)) {
    changes.push({ field: "recurrence" });
  }
  if (Boolean(before.discord_linked) !== Boolean(after.discord_linked)) {
    changes.push({ field: "discord", after: Boolean(after.discord_linked) });
  }
  return changes;
}

/**
 * 名前を引く操作者 (重複なし、新しい順)。
 * 管理コンソールの操作者はサーバーのメンバーとは限らないので引かない (表示は「運営」)
 */
export function historyActorIds(entries: EventHistoryEntry[]): string[] {
  const ids: string[] = [];
  for (const entry of entries) {
    if (entry.source === "admin") continue;
    const id = entry.actor_discord_user_id;
    if (id && !ids.includes(id)) ids.push(id);
  }
  return ids;
}

/** 操作した時刻 (UTC) を、予定の日時と同じく JST の「M/D HH:mm」で表す */
export function formatHistoryTime(createdAt: string): string {
  const time = new Date(createdAt);
  if (Number.isNaN(time.getTime())) return createdAt;
  const jst = new Date(time.getTime() + 9 * 60 * 60 * 1000);
  const hours = String(jst.getUTCHours()).padStart(2, "0");
  const minutes = String(jst.getUTCMinutes()).padStart(2, "0");
  return `${jst.getUTCFullYear()}/${jst.getUTCMonth() + 1}/${jst.getUTCDate()} ${hours}:${minutes}`;
}
