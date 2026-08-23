import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import { revalidateAdminPagesQuietly } from "./admin-cache";
import { queryKeys } from "./keys";

// 管理コンソールのユーザー・セッションと差分検出 (#37)。
// 一覧 (/admin/users) は RSC が描画するので、ここにあるのは押したときだけ動くものと詳細だけ

/**
 * Bot の参加ギルド (Discord API) と guilds テーブルの差分。
 * Discord を全ギルド分辿る重い呼び出しなので、画面を開いただけでは実行せず `enabled` で明示的に始める
 */
export function useSyncCheck(enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.admin.syncCheck,
    queryFn: () => api.admin.guilds.syncCheck(),
    enabled,
    // 押したときの状態を見たいので、古い結果は使わない (api 側にも 60 秒のキャッシュがある)
    staleTime: 0,
    // 画面を離れたら結果を捨てる。残しておくと、戻ってきたときに (まだ押していないのに)
    // 前回の差分がキャッシュから復元されて現在の状態のように見えてしまう
    gcTime: 0,
  });
}

/** あるユーザーのセッション一覧 (トークンは含まれない)。開いたときだけ取る */
export function useAdminUserSessions(userId: string, enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.admin.userSessions(userId),
    queryFn: () => api.admin.users.sessions(userId),
    enabled,
  });
}

/**
 * 強制ログアウト (セッションの全削除、監査ログに残る)。
 * セッション一覧を取り直し、RSC のユーザー一覧 (セッション数) も再取得させる
 */
export function useRevokeUserSessions(userId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.admin.users.revokeSessions(userId),
    onSuccess: () => {
      queryClient.setQueryData(queryKeys.admin.userSessions(userId), []);
      revalidateAdminPagesQuietly();
    },
  });
}
