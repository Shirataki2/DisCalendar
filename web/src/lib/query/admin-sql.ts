import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { adminEventsSource, invalidateEvents } from "./events";
import { queryKeys } from "./keys";

// 管理コンソールの SQL コンソールと定型操作 (#36)

/** 直近の実行履歴 (監査ログの sql.select、全管理者分 20 件) */
export function useSqlHistory() {
  return useQuery({
    queryKey: queryKeys.admin.sqlHistory,
    queryFn: () => api.admin.sql.history(),
  });
}

/**
 * 読み取り専用 SQL の実行。結果は呼び出し側が state に持つ (キャッシュしない)。
 * 成功・失敗どちらも監査ログに残るので、終わったら履歴を取り直す
 */
export function useRunSql() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (sql: string) => api.admin.sql.run(sql),
    onSettled: () =>
      queryClient.invalidateQueries({ queryKey: queryKeys.admin.sqlHistory }),
  });
}

/**
 * 指定ギルドの予定をすべて削除する (監査ログに削除した予定が残る)。
 * 通常画面・管理画面の予定一覧とギルド詳細 (予定数)、RSC のギルド一覧を古いままにしない
 */
export function useDeleteGuildEvents() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (guildId: string) => api.admin.ops.deleteGuildEvents(guildId),
    onSuccess: (_result, guildId) =>
      invalidateEvents(queryClient, adminEventsSource, guildId, true),
  });
}

/** Better Auth の期限切れセッションの削除 */
export function usePurgeExpiredSessions() {
  return useMutation({
    mutationFn: () => api.admin.ops.purgeExpiredSessions(),
  });
}
