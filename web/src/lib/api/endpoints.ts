import type { ApiFetcher } from "./client";
import type {
  AdminAnalytics,
  AdminAuditLogPage,
  AdminGuildDetail,
  AdminGuildPage,
  AdminGuildSyncCheck,
  AdminMe,
  AdminSession,
  AdminStats,
  AdminStatus,
  AdminUserPage,
  ApiEvent,
  ApiEventInput,
  Guild,
  GuildConfig,
  GuildFeed,
  MemberProfile,
  MyPermissions,
  OpsResult,
  ShareLink,
  SqlHistoryEntry,
  SqlResult,
} from "./types";

/**
 * 予定の CRUD。通常の `/events/{guild_id}` と管理コンソールの `/admin/guilds/{guild_id}/events` は
 * 入出力が同じなので、ベースパスだけ差し替えて同じ形のクライアントを作る (カレンダー UI を両方で使い回すため)
 */
function createEventsClient(
  request: ApiFetcher,
  base: (guildId: string) => string,
) {
  return {
    /** `[start, end)` (JST 文字列) に重なる予定 */
    list: (
      guildId: string,
      start: string,
      end: string,
      signal?: AbortSignal,
    ) => {
      const query = new URLSearchParams({ start, end });
      return request<ApiEvent[]>(`${base(guildId)}?${query}`, { signal });
    },
    create: (guildId: string, input: ApiEventInput) =>
      request<ApiEvent>(base(guildId), { method: "POST", body: input }),
    update: (guildId: string, eventId: number, input: ApiEventInput) =>
      request<ApiEvent>(`${base(guildId)}/${eventId}`, {
        method: "PUT",
        body: input,
      }),
    remove: (guildId: string, eventId: number) =>
      request<void>(`${base(guildId)}/${eventId}`, { method: "DELETE" }),
  };
}

export type EventsClient = ReturnType<typeof createEventsClient>;

/**
 * API のエンドポイント定義。呼び出し方 (ブラウザ経由のプロキシ / RSC からの直接呼び出し) は
 * fetcher に委ねる。パスは api/src/routes と対応
 */
export function createApi(request: ApiFetcher) {
  return {
    shares: {
      get: (guildId: string, eventId: number) =>
        request<ShareLink | null>(`/events/${guildId}/${eventId}/share`),
      issue: (guildId: string, eventId: number) =>
        request<ShareLink>(`/events/${guildId}/${eventId}/share`, {
          method: "POST",
        }),
      revoke: (guildId: string, eventId: number) =>
        request<void>(`/events/${guildId}/${eventId}/share`, {
          method: "DELETE",
        }),
    },
    guilds: {
      members: (guildId: string, ids: string[], signal?: AbortSignal) => {
        const query = new URLSearchParams({ ids: ids.join(",") });
        return request<MemberProfile[]>(`/guilds/${guildId}/members?${query}`, {
          signal,
        });
      },
      /** 指定したギルドのうち Bot が参加しているもの */
      joined: (guildIds: string[]) => {
        const query = new URLSearchParams({ guild_ids: guildIds.join(",") });
        return request<Guild[]>(`/guilds/joined?${query}`);
      },
      get: (guildId: string) => request<Guild>(`/guilds/${guildId}`),
      config: (guildId: string) =>
        request<GuildConfig>(`/guilds/${guildId}/config`),
      updateConfig: (guildId: string, restricted: boolean) =>
        request<GuildConfig>(`/guilds/${guildId}/config`, {
          method: "PUT",
          body: { restricted },
        }),
      myPermissions: (guildId: string) =>
        request<MyPermissions>(`/guilds/${guildId}/@me/permissions`),
      /**
       * 権限のキャッシュを捨てて取り直す (#122)。Bot を招待し直したりロールを付けてもらった直後に、
       * api 側のキャッシュ (最大 5 分) の期限を待たずに反映させる
       */
      refreshMyPermissions: (guildId: string) =>
        request<MyPermissions>(`/guilds/${guildId}/@me/permissions/refresh`, {
          method: "POST",
        }),
      /** iCal フィード (#95) の発行状況。メンバーなら誰でも見られる。未発行なら null */
      feed: (guildId: string) =>
        request<GuildFeed | null>(`/guilds/${guildId}/feed`),
      /** フィードの発行・再発行 (管理権限が必要。発行済みなら新しい URL に置き換わり、古い URL は使えなくなる) */
      issueFeed: (guildId: string) =>
        request<GuildFeed>(`/guilds/${guildId}/feed`, { method: "POST" }),
      /** フィードの無効化 (管理権限が必要) */
      revokeFeed: (guildId: string) =>
        request<void>(`/guilds/${guildId}/feed`, { method: "DELETE" }),
    },
    events: createEventsClient(request, (guildId) => `/events/${guildId}`),
    /**
     * 参加している複数サーバーの予定をまとめて取る (横断カレンダー #98)。閲覧専用で書き込みは無い。
     * guildIds は Bot 参加済みのもの (`guilds.joined` の結果) を渡す。メンバーでないサーバーは api が除外する。
     * 一度に渡せるのは JOINED_EVENTS_MAX_GUILDS 件まで
     */
    joinedEvents: {
      /** `[start, end)` (JST 文字列) に重なる予定 */
      list: (
        guildIds: readonly string[],
        start: string,
        end: string,
        signal?: AbortSignal,
      ) => {
        const query = new URLSearchParams({
          guild_ids: guildIds.join(","),
          start,
          end,
        });
        return request<ApiEvent[]>(`/events/@me?${query}`, { signal });
      },
    },
    /** 管理コンソール (api/src/routes/admin*.rs)。管理者以外は 403 */
    admin: {
      me: () => request<AdminMe>("/admin/me"),
      /** 概要の件数と直近のギルドの出入り (#37) */
      stats: () => request<AdminStats>("/admin/stats"),
      /** DB 疎通・マイグレーション・ビルド情報 (#37) */
      status: () => request<AdminStatus>("/admin/status"),
      /** アクティブユーザー・予定の作成数とその推移 (#79)。stats より重い */
      analytics: () => request<AdminAnalytics>("/admin/analytics"),
      guilds: {
        members: (guildId: string, ids: string[], signal?: AbortSignal) => {
          const query = new URLSearchParams({ ids: ids.join(",") });
          return request<MemberProfile[]>(
            `/guilds/${guildId}/members?${query}`,
            { signal },
          );
        },
        /** 全ギルドの一覧・検索 (q: guild_id の完全一致 or 名前の部分一致、page: 1 始まり) */
        list: (q: string, page: number) => {
          const query = new URLSearchParams({ q, page: String(page) });
          return request<AdminGuildPage>(`/admin/guilds?${query}`);
        },
        get: (guildId: string) =>
          request<AdminGuildDetail>(`/admin/guilds/${guildId}`),
        /** Bot の参加ギルド (Discord API) と guilds テーブルの差分 (#37) */
        syncCheck: () =>
          request<AdminGuildSyncCheck>("/admin/guilds/sync-check"),
        /** restricted の切替 (監査ログに残る) */
        updateConfig: (guildId: string, restricted: boolean) =>
          request<GuildConfig>(`/admin/guilds/${guildId}/config`, {
            method: "PUT",
            body: { restricted },
          }),
      },
      /** 任意のギルドの予定 (メンバーシップに関係なく扱える。書き込みは監査ログに残る) */
      events: createEventsClient(
        request,
        (guildId) => `/admin/guilds/${guildId}/events`,
      ),
      /** 読み取り専用 SQL コンソール (#36)。実行は成功・失敗とも監査ログに残る */
      sql: {
        /** 実行できない文や Postgres のエラーは 400 (bad_request) で、message にそのまま入る */
        run: (sql: string) =>
          request<SqlResult>("/admin/sql", { method: "POST", body: { sql } }),
        history: () => request<SqlHistoryEntry[]>("/admin/sql/history"),
      },
      /** 定型の書き込み操作 (#36)。すべて監査ログに残る */
      ops: {
        /** 指定ギルドの予定をすべて削除する (未知のギルドは 404) */
        deleteGuildEvents: (guildId: string) =>
          request<OpsResult>("/admin/ops/delete-guild-events", {
            method: "POST",
            body: { guild_id: guildId },
          }),
        /** Better Auth の期限切れセッションを削除する */
        purgeExpiredSessions: () =>
          request<OpsResult>("/admin/ops/purge-expired-sessions", {
            method: "POST",
          }),
      },
      /** ユーザーとセッション (#37)。セッショントークンは返らない */
      users: {
        /** 一覧・検索 (q: user.id / Discord ID の完全一致か名前・メールの部分一致) */
        list: (q: string, page: number) => {
          const query = new URLSearchParams({ q, page: String(page) });
          return request<AdminUserPage>(`/admin/users?${query}`);
        },
        sessions: (userId: string) =>
          request<AdminSession[]>(
            `/admin/users/${encodeURIComponent(userId)}/sessions`,
          ),
        /** 強制ログアウト (全セッションの削除。監査ログに残る) */
        revokeSessions: (userId: string) =>
          request<OpsResult>(
            `/admin/users/${encodeURIComponent(userId)}/sessions`,
            { method: "DELETE" },
          ),
      },
      /** 監査ログの閲覧 (#37) */
      auditLogs: {
        list: (action: string, actor: string, page: number) => {
          const query = new URLSearchParams({
            action,
            actor,
            page: String(page),
          });
          return request<AdminAuditLogPage>(`/admin/audit-logs?${query}`);
        },
      },
    },
  };
}

export type Api = ReturnType<typeof createApi>;
