import type { ApiFetcher } from "./client";
import type {
  ApiEvent,
  ApiEventInput,
  Guild,
  GuildConfig,
  MyPermissions,
} from "./types";

/**
 * API のエンドポイント定義。呼び出し方 (ブラウザ経由のプロキシ / RSC からの直接呼び出し) は
 * fetcher に委ねる。パスは api/src/routes と対応
 */
export function createApi(request: ApiFetcher) {
  return {
    guilds: {
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
    },
    events: {
      /** `[start, end)` (JST 文字列) に重なる予定 */
      list: (
        guildId: string,
        start: string,
        end: string,
        signal?: AbortSignal,
      ) => {
        const query = new URLSearchParams({ start, end });
        return request<ApiEvent[]>(`/events/${guildId}?${query}`, { signal });
      },
      create: (guildId: string, input: ApiEventInput) =>
        request<ApiEvent>(`/events/${guildId}`, {
          method: "POST",
          body: input,
        }),
      update: (guildId: string, eventId: number, input: ApiEventInput) =>
        request<ApiEvent>(`/events/${guildId}/${eventId}`, {
          method: "PUT",
          body: input,
        }),
      remove: (guildId: string, eventId: number) =>
        request<void>(`/events/${guildId}/${eventId}`, { method: "DELETE" }),
    },
  };
}

export type Api = ReturnType<typeof createApi>;
