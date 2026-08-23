import {
  keepPreviousData,
  type QueryKey,
  skipToken,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { revalidateAdminPages } from "@/app/admin/guilds/actions";
import { api } from "@/lib/api";
import type { EventsClient } from "@/lib/api/endpoints";
import type { ApiEvent, ApiEventInput } from "@/lib/api/types";
import { type EventsQueryKeys, queryKeys } from "./keys";

export interface EventRange {
  /** JST 文字列 (含む) */
  start: string;
  /** JST 文字列 (含まない) */
  end: string;
}

/**
 * 予定の取得元。通常画面はユーザーのギルドの `/events/{guild_id}`、管理コンソールは
 * `/admin/guilds/{guild_id}/events` を使い、
 * キャッシュも別のキーに持つ (同じギルドを両方で開いても混ざらない)
 */
export interface EventsSource {
  client: EventsClient;
  keys: EventsQueryKeys;
  /**
   * 予定の件数が変わった (作成・削除した) 後に追加で行うこと。
   * TanStack Query の外にあるキャッシュ (RSC が描画する画面の Router Cache など) を捨てるのに使う
   */
  afterCountChanged?: () => Promise<void>;
}

export const dashboardEventsSource: EventsSource = {
  client: api.events,
  keys: queryKeys.events,
};

export const adminEventsSource: EventsSource = {
  client: api.admin.events,
  keys: queryKeys.admin.events,
  // ギルド一覧 (RSC) の予定数を古いままにしない
  afterCountChanged: revalidateAdminPages,
};

/** 表示範囲に重なる予定。範囲が決まるまで (FullCalendar の datesSet 前) は取得しない */
export function useEventsQuery(
  guildId: string,
  range: EventRange | null,
  { client, keys }: EventsSource = dashboardEventsSource,
) {
  return useQuery({
    queryKey: range
      ? keys.range(guildId, range.start, range.end)
      : keys.all(guildId),
    queryFn: range
      ? ({ signal }) => client.list(guildId, range.start, range.end, signal)
      : skipToken,
    // 月を移動したときに前の月の予定を出したまま次を読み込む (カレンダーが空にならない)
    placeholderData: keepPreviousData,
  });
}

/**
 * 予定一覧と、同じ予定を見ている他のキャッシュ (`keys.onChanged`: もう一方の一覧、
 * `keys.onCountChanged`: 件数に依存するもの) をまとめて無効化する
 */
function invalidateEvents(
  queryClient: ReturnType<typeof useQueryClient>,
  { keys, afterCountChanged }: EventsSource,
  guildId: string,
  countChanged: boolean,
) {
  const targets = [keys.all(guildId), ...(keys.onChanged?.(guildId) ?? [])];
  if (countChanged) targets.push(...(keys.onCountChanged?.(guildId) ?? []));
  return Promise.all([
    ...targets.map((queryKey) => queryClient.invalidateQueries({ queryKey })),
    countChanged ? afterCountChanged?.() : undefined,
  ]);
}

export function useCreateEvent(
  guildId: string,
  source: EventsSource = dashboardEventsSource,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: ApiEventInput) => source.client.create(guildId, input),
    onSuccess: () => invalidateEvents(queryClient, source, guildId, true),
  });
}

type CachedLists = [QueryKey, ApiEvent[] | undefined][];

/**
 * 予定の更新。ドラッグ / リサイズの即時反映のため、キャッシュ上の予定を先に書き換え (楽観的更新)、
 * 失敗したら元に戻す。呼び出し側は onError で FullCalendar 側も revert すること
 */
export function useUpdateEvent(
  guildId: string,
  source: EventsSource = dashboardEventsSource,
) {
  const queryClient = useQueryClient();
  const listsKey = source.keys.all(guildId);
  return useMutation({
    mutationFn: ({ id, input }: { id: number; input: ApiEventInput }) =>
      source.client.update(guildId, id, input),
    onMutate: async ({ id, input }) => {
      // 進行中の取得が古い状態で上書きしないよう止める
      await queryClient.cancelQueries({ queryKey: listsKey });
      const previous: CachedLists = queryClient.getQueriesData<ApiEvent[]>({
        queryKey: listsKey,
      });
      queryClient.setQueriesData<ApiEvent[]>({ queryKey: listsKey }, (events) =>
        events?.map((event) =>
          event.id === id
            ? { ...event, ...input, description: input.description ?? null }
            : event,
        ),
      );
      return { previous };
    },
    onError: (_error, _variables, context) => {
      for (const [key, data] of context?.previous ?? []) {
        queryClient.setQueryData(key, data);
      }
    },
    onSettled: () => invalidateEvents(queryClient, source, guildId, false),
  });
}

export function useDeleteEvent(
  guildId: string,
  source: EventsSource = dashboardEventsSource,
) {
  const queryClient = useQueryClient();
  const listsKey = source.keys.all(guildId);
  return useMutation({
    mutationFn: (id: number) => source.client.remove(guildId, id),
    onMutate: async (id) => {
      await queryClient.cancelQueries({ queryKey: listsKey });
      const previous: CachedLists = queryClient.getQueriesData<ApiEvent[]>({
        queryKey: listsKey,
      });
      queryClient.setQueriesData<ApiEvent[]>({ queryKey: listsKey }, (events) =>
        events?.filter((event) => event.id !== id),
      );
      return { previous };
    },
    onError: (_error, _id, context) => {
      for (const [key, data] of context?.previous ?? []) {
        queryClient.setQueryData(key, data);
      }
    },
    onSettled: () => invalidateEvents(queryClient, source, guildId, true),
  });
}
