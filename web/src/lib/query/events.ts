import {
  keepPreviousData,
  type QueryKey,
  skipToken,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { ApiError, api } from "@/lib/api";
import type { EventsClient } from "@/lib/api/endpoints";
import type { ApiEvent, ApiEventInput } from "@/lib/api/types";
import { revalidateAdminPagesQuietly } from "./admin-cache";
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
   * TanStack Query の外にあるキャッシュ (RSC が描画する画面の Router Cache など) を捨てるのに使う。
   * 書き込み本体は成功済みなので、ここでの失敗は mutation の結果に影響させない (返り値なし)
   */
  afterCountChanged?: () => void;
}

// どちらの画面で予定を作成・削除しても、管理コンソールのギルド一覧 (RSC) の予定数を古いままにしない
// (管理者でない利用者からも呼ばれるが、Router Cache を捨てるだけなので害はない)
export const dashboardEventsSource: EventsSource = {
  client: api.events,
  keys: queryKeys.events,
  afterCountChanged: revalidateAdminPagesQuietly,
};

export const adminEventsSource: EventsSource = {
  client: api.admin.events,
  keys: queryKeys.admin.events,
  afterCountChanged: revalidateAdminPagesQuietly,
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
 * `keys.onCountChanged`: 件数に依存するもの) をまとめて無効化する。
 * 管理コンソールの定型操作 (全予定削除、lib/query/admin-sql.ts) からも使う
 */
export function invalidateEvents(
  queryClient: ReturnType<typeof useQueryClient>,
  { keys, afterCountChanged }: EventsSource,
  guildId: string,
  countChanged: boolean,
) {
  const targets = [keys.all(guildId), ...(keys.onChanged?.(guildId) ?? [])];
  if (countChanged) {
    targets.push(...(keys.onCountChanged?.(guildId) ?? []));
    afterCountChanged?.();
  }
  return Promise.all(
    targets.map((queryKey) => queryClient.invalidateQueries({ queryKey })),
  );
}

/**
 * 自分の権限を取り直す (#122)。api は Discord から 403 を返された時点で権限キャッシュを
 * 捨てているので、取り直すと今の状態になる。これをしないと、開いたままのダイアログは
 * `bot_create_events: true` のままでチェックボックスが有効に見え、「権限を再確認」も出ない
 */
function refetchPermissions(
  queryClient: ReturnType<typeof useQueryClient>,
  guildId: string,
) {
  void queryClient.invalidateQueries({
    queryKey: queryKeys.guild.myPermissions(guildId),
  });
}

/** Bot の権限不足 (403 `bot_permission`) で失敗したときに取り直す */
function refetchPermissionsOnBotError(
  queryClient: ReturnType<typeof useQueryClient>,
  guildId: string,
  error: unknown,
) {
  if (error instanceof ApiError && error.kind === "bot_permission") {
    refetchPermissions(queryClient, guildId);
  }
}

/**
 * 連携していた予定の連携が外れた (解除・作り直し・削除) ときにも取り直す (#122)。
 *
 * 不要になった Discord イベントの後始末は api ではベストエフォートで、権限不足で消せなくても
 * 予定の操作自体は成功として返る。つまりブラウザには 403 が伝わらないので、
 * {@link refetchPermissionsOnBotError} だけでは古い「権限あり」が残ってしまう
 */
function refetchPermissionsIfUnlinked(
  queryClient: ReturnType<typeof useQueryClient>,
  guildId: string,
  linkedIdBefore: string | null,
  linkedIdAfter: string | null,
) {
  if (linkedIdBefore !== null && linkedIdBefore !== linkedIdAfter) {
    refetchPermissions(queryClient, guildId);
  }
}

/** キャッシュ上のこの予定が指している Discord イベントの ID (連携していなければ null) */
function linkedIdOf(
  queryClient: ReturnType<typeof useQueryClient>,
  listsKey: QueryKey,
  id: number,
): string | null {
  for (const [, events] of queryClient.getQueriesData<ApiEvent[]>({
    queryKey: listsKey,
  })) {
    const found = events?.find((event) => event.id === id);
    if (found) return found.discord_scheduled_event_id;
  }
  return null;
}

export function useCreateEvent(
  guildId: string,
  source: EventsSource = dashboardEventsSource,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: ApiEventInput) => source.client.create(guildId, input),
    onSuccess: () => invalidateEvents(queryClient, source, guildId, true),
    onError: (error) =>
      refetchPermissionsOnBotError(queryClient, guildId, error),
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
      const linkedIdBefore = linkedIdOf(queryClient, listsKey, id);
      const previous: CachedLists = queryClient.getQueriesData<ApiEvent[]>({
        queryKey: listsKey,
      });
      // discord_scheduled_event はリクエスト専用のフラグで ApiEvent には無いので剥がす
      // (連携 ID の変化は成功後に onSuccess でレスポンスを反映する)
      const { discord_scheduled_event: _flag, ...rest } = input;
      queryClient.setQueriesData<ApiEvent[]>({ queryKey: listsKey }, (events) =>
        events?.map((event) =>
          event.id === id
            ? { ...event, ...rest, description: input.description ?? null }
            : event,
        ),
      );
      return { previous, linkedIdBefore };
    },
    // サーバーが返した内容でキャッシュを揃える。とくに Discord の連携 ID (#94) は
    // 楽観的更新では分からないので、ここで入れておかないと再取得が失敗したときに
    // 古い ID が残り、次の編集で連携を意図せず解除・作り直ししてしまう
    onSuccess: (updated, _variables, context) => {
      queryClient.setQueriesData<ApiEvent[]>({ queryKey: listsKey }, (events) =>
        events?.map((event) => (event.id === updated.id ? updated : event)),
      );
      refetchPermissionsIfUnlinked(
        queryClient,
        guildId,
        context?.linkedIdBefore ?? null,
        updated.discord_scheduled_event_id,
      );
    },
    onError: (error, _variables, context) => {
      for (const [key, data] of context?.previous ?? []) {
        queryClient.setQueryData(key, data);
      }
      refetchPermissionsOnBotError(queryClient, guildId, error);
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
      const linkedIdBefore = linkedIdOf(queryClient, listsKey, id);
      const previous: CachedLists = queryClient.getQueriesData<ApiEvent[]>({
        queryKey: listsKey,
      });
      queryClient.setQueriesData<ApiEvent[]>({ queryKey: listsKey }, (events) =>
        events?.filter((event) => event.id !== id),
      );
      return { previous, linkedIdBefore };
    },
    // 消した予定が連携していたなら、Discord 側の後始末が権限不足で失敗していることがある
    onSuccess: (_data, _id, context) =>
      refetchPermissionsIfUnlinked(
        queryClient,
        guildId,
        context?.linkedIdBefore ?? null,
        null,
      ),
    onError: (error, _id, context) => {
      for (const [key, data] of context?.previous ?? []) {
        queryClient.setQueryData(key, data);
      }
      refetchPermissionsOnBotError(queryClient, guildId, error);
    },
    onSettled: () => invalidateEvents(queryClient, source, guildId, true),
  });
}
