import type { QueryKey } from "@tanstack/react-query";

/** TanStack Query のキー。無効化は前方一致なので、ギルド単位でまとめて無効化できる */
export const queryKeys = {
  events: {
    all: (guildId: string) => ["events", guildId] as const,
    range: (guildId: string, start: string, end: string) =>
      ["events", guildId, { start, end }] as const,
    // 管理コンソールも同じ events 行を見ているので、こちらで変更したらあちらの一覧も古くなる。
    // 横断カレンダー (#98) は全ギルドの予定をまとめて持つので、どのギルドの変更でも捨てる
    onChanged: (guildId: string) => [
      ["admin", "events", guildId],
      ["events", "joined"],
    ],
    onCountChanged: (guildId: string) => [["admin", "guild", guildId]],
  },
  /**
   * 横断カレンダー (#98) の予定一覧。"joined" は Snowflake ではないので、
   * ギルド単位の `["events", guildId]` の前方一致とは混ざらない
   */
  joinedEvents: {
    all: ["events", "joined"] as const,
    range: (guildIds: readonly string[], start: string, end: string) =>
      ["events", "joined", { guildIds, start, end }] as const,
  },
  guild: {
    members: (guildId: string, ids: string[]) =>
      ["guild", guildId, "members", ids] as const,
    detail: (guildId: string) => ["guild", guildId] as const,
    config: (guildId: string) => ["guild", guildId, "config"] as const,
    myPermissions: (guildId: string) =>
      ["guild", guildId, "permissions"] as const,
    /** iCal フィードの発行状況 (#95) */
    feed: (guildId: string) => ["guild", guildId, "feed"] as const,
  },
  /** 管理コンソール。通常画面とは別のエンドポイントなので、キャッシュも別に持つ */
  admin: {
    guild: (guildId: string) => ["admin", "guild", guildId] as const,
    /** SQL コンソールの実行履歴 (監査ログ) */
    sqlHistory: ["admin", "sql", "history"] as const,
    /** Bot の参加ギルドと guilds テーブルの差分 (#37) */
    syncCheck: ["admin", "guilds", "sync-check"] as const,
    /** あるユーザーのセッション一覧 (#37) */
    userSessions: (userId: string) =>
      ["admin", "user", userId, "sessions"] as const,
    events: {
      all: (guildId: string) => ["admin", "events", guildId] as const,
      range: (guildId: string, start: string, end: string) =>
        ["admin", "events", guildId, { start, end }] as const,
      // 通常画面 (/dashboard/[id]) と横断カレンダー (#98) の一覧を古いままにしない
      onChanged: (guildId: string) => [
        ["events", guildId],
        ["events", "joined"],
      ],
      // ギルド詳細の event_count を古いままにしない
      onCountChanged: (guildId: string) => [["admin", "guild", guildId]],
    },
  },
};

/**
 * 予定一覧のキー。通常画面 (`queryKeys.events`) と管理コンソール (`queryKeys.admin.events`) で同じ形。
 * キャッシュは分けているが DB 上は同じ予定なので、mutation 後はもう一方も無効化して
 * (同じ QueryClient のまま画面を行き来しても) 古い予定が出ないようにする
 */
export interface EventsQueryKeys {
  all: (guildId: string) => QueryKey;
  range: (guildId: string, start: string, end: string) => QueryKey;
  /** 予定が変わったとき (作成・更新・削除) に一覧と一緒に無効化するキー (もう一方の予定一覧) */
  onChanged?: (guildId: string) => QueryKey[];
  /** 予定の件数が変わったとき (作成・削除) にさらに無効化するキー (管理コンソールのギルド詳細など) */
  onCountChanged?: (guildId: string) => QueryKey[];
}
