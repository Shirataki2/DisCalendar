import {
  keepPreviousData,
  type QueryKey,
  skipToken,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { ApiEvent, ApiEventInput } from "@/lib/api/types";
import { queryKeys } from "./keys";

export interface EventRange {
  /** JST 文字列 (含む) */
  start: string;
  /** JST 文字列 (含まない) */
  end: string;
}

/** 表示範囲に重なる予定。範囲が決まるまで (FullCalendar の datesSet 前) は取得しない */
export function useEventsQuery(guildId: string, range: EventRange | null) {
  return useQuery({
    queryKey: range
      ? queryKeys.events.range(guildId, range.start, range.end)
      : queryKeys.events.all(guildId),
    queryFn: range
      ? ({ signal }) => api.events.list(guildId, range.start, range.end, signal)
      : skipToken,
    // 月を移動したときに前の月の予定を出したまま次を読み込む (カレンダーが空にならない)
    placeholderData: keepPreviousData,
  });
}

export function useCreateEvent(guildId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: ApiEventInput) => api.events.create(guildId, input),
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: queryKeys.events.all(guildId),
      }),
  });
}

type CachedLists = [QueryKey, ApiEvent[] | undefined][];

/**
 * 予定の更新。ドラッグ / リサイズの即時反映のため、キャッシュ上の予定を先に書き換え (楽観的更新)、
 * 失敗したら元に戻す。呼び出し側は onError で FullCalendar 側も revert すること
 */
export function useUpdateEvent(guildId: string) {
  const queryClient = useQueryClient();
  const listsKey = queryKeys.events.all(guildId);
  return useMutation({
    mutationFn: ({ id, input }: { id: number; input: ApiEventInput }) =>
      api.events.update(guildId, id, input),
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
    onSettled: () => queryClient.invalidateQueries({ queryKey: listsKey }),
  });
}

export function useDeleteEvent(guildId: string) {
  const queryClient = useQueryClient();
  const listsKey = queryKeys.events.all(guildId);
  return useMutation({
    mutationFn: (id: number) => api.events.remove(guildId, id),
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
    onSettled: () => queryClient.invalidateQueries({ queryKey: listsKey }),
  });
}
