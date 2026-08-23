import type { QueryKey } from "@tanstack/react-query";

/** TanStack Query のキー。無効化は前方一致なので、ギルド単位でまとめて無効化できる */
export const queryKeys = {
  events: {
    all: (guildId: string) => ["events", guildId] as const,
    range: (guildId: string, start: string, end: string) =>
      ["events", guildId, { start, end }] as const,
  },
  guild: {
    detail: (guildId: string) => ["guild", guildId] as const,
    config: (guildId: string) => ["guild", guildId, "config"] as const,
    myPermissions: (guildId: string) =>
      ["guild", guildId, "permissions"] as const,
  },
  /** 管理コンソール。通常画面とは別のエンドポイントなので、キャッシュも別に持つ */
  admin: {
    guild: (guildId: string) => ["admin", "guild", guildId] as const,
    events: {
      all: (guildId: string) => ["admin", "events", guildId] as const,
      range: (guildId: string, start: string, end: string) =>
        ["admin", "events", guildId, { start, end }] as const,
    },
  },
};

/** 予定一覧のキー。通常画面 (`queryKeys.events`) と管理コンソール (`queryKeys.admin.events`) で同じ形 */
export interface EventsQueryKeys {
  all: (guildId: string) => QueryKey;
  range: (guildId: string, start: string, end: string) => QueryKey;
}
