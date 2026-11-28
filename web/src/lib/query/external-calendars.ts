import {
  skipToken,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useState } from "react";
import { api } from "@/lib/api";
import type { ExternalCalendar, ExternalCalendarInput } from "@/lib/api/types";
import type { EventRange } from "./events";
import { queryKeys } from "./keys";

export function useExternalDisplayErrors(guildId: string) {
  return useQuery<Record<number, string>>({
    queryKey: queryKeys.external.displayErrors(guildId),
    queryFn: skipToken,
  });
}

/** まだ一度も取得していない購読先があるときに、取得状態を取り直す間隔 */
const PENDING_FETCH_POLL_MS = 3000;
/**
 * 取り直しを続ける長さ。ICS を取得するのはカレンダーの表示と「今すぐ取得」(と登録・URL 変更の直後) だけで、
 * 一覧の取得では進まないので、どこからも取得が走っていなければ待っても変わらない。開いてからこの時間で打ち切る
 */
const PENDING_FETCH_POLL_LIMIT_MS = 60_000;

/** 一度も取得しておらず、エラーも無い (取得待ちの) 購読先か */
function isPendingFetch(calendar: ExternalCalendar): boolean {
  return !calendar.last_fetched_at && !calendar.last_error;
}

/**
 * 登録済みの外部カレンダー (サーバー設定の「外部カレンダー」)。
 * 設定はカレンダーとは別のページなので、カレンダー側の取得が終わっても知らせが届かない。
 * まだ一度も取得していない (結果もエラーも無い) 購読先があるうちは、取得状態を数秒おきに取り直す
 */
export function useExternalCalendars(guildId: string) {
  const [mountedAt] = useState(() => Date.now());
  return useQuery({
    queryKey: queryKeys.guild.externalCalendars(guildId),
    queryFn: () => api.guilds.externalCalendars(guildId),
    refetchInterval: (query) =>
      query.state.data?.some(isPendingFetch) &&
      Date.now() - mountedAt < PENDING_FETCH_POLL_LIMIT_MS
        ? PENDING_FETCH_POLL_MS
        : false,
  });
}

export function useExternalEvents(
  guildId: string,
  range: EventRange | null,
  enabled = true,
) {
  const client = useQueryClient();
  return useQuery({
    queryKey: range
      ? queryKeys.external.range(guildId, range.start, range.end)
      : queryKeys.external.all(guildId),
    queryFn:
      range && enabled
        ? async ({ signal }) => {
            const result = await api.externalEvents(
              guildId,
              range.start,
              range.end,
              signal,
            );
            client.setQueryData(
              queryKeys.external.displayErrors(guildId),
              Object.fromEntries(
                result
                  .filter(({ calendar }) => calendar.last_error)
                  .map(({ calendar }) => [calendar.id, calendar.last_error]),
              ),
            );
            await client.invalidateQueries({
              queryKey: queryKeys.guild.externalCalendars(guildId),
            });
            return result;
          }
        : skipToken,
  });
}

export function useExternalCalendarActions(guildId: string) {
  const client = useQueryClient();
  const refresh = async () => {
    await Promise.all([
      client.invalidateQueries({
        queryKey: queryKeys.guild.externalCalendars(guildId),
      }),
      client.invalidateQueries({ queryKey: queryKeys.external.all(guildId) }),
    ]);
  };
  /**
   * 登録・URL 変更の直後で未取得の購読先は、その場で最初の取得を始める。
   * 設定はカレンダーと別のページなので、カレンダー側の表示による取得を待つと「まだ取得していません」のままになる。
   * 遅い ICS で追加・保存の完了が遅れないよう、取得は待たずに裏で進め、終わったら一覧と表示を取り直す
   */
  const startFirstFetch = (calendar: ExternalCalendar) => {
    if (!isPendingFetch(calendar)) return;
    void api.guilds
      .refreshExternalCalendar(guildId, calendar.id)
      // 取得の失敗は購読先の last_error として一覧に出るので、ここでは握りつぶして取り直すだけにする
      .catch(() => undefined)
      .finally(() => void refresh());
  };
  const add = useMutation({
    mutationFn: (input: ExternalCalendarInput) =>
      api.guilds.addExternalCalendar(guildId, input),
    onSuccess: async (calendar) => {
      startFirstFetch(calendar);
      await refresh();
    },
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: number; input: ExternalCalendarInput }) =>
      api.guilds.updateExternalCalendar(guildId, id, input),
    // URL を変えると API が取得状態を消す (last_fetched_at / last_error が NULL に戻る)
    onSuccess: async (calendar) => {
      startFirstFetch(calendar);
      await refresh();
    },
  });
  const remove = useMutation({
    mutationFn: (id: number) => api.guilds.removeExternalCalendar(guildId, id),
    onSuccess: refresh,
  });
  const fetchNow = useMutation({
    mutationFn: (id: number) => api.guilds.refreshExternalCalendar(guildId, id),
    onSuccess: refresh,
  });
  return { add, update, remove, fetchNow };
}
