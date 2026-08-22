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
};
