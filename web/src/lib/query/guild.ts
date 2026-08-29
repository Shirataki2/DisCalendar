import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { GuildConfig, MyPermissions } from "@/lib/api/types";
import { syncAdminGuildConfig } from "./admin-cache";
import { queryKeys } from "./keys";

// 初回の値は dashboard/[id]/page.tsx (RSC) が取得して hydrate するので、
// クライアントでは再取得 (サーバー設定ダイアログの「再読込」や staleTime 経過後) にだけ API を呼ぶ

/** ギルド設定 (restricted かどうか) */
export function useGuildConfigQuery(guildId: string) {
  return useQuery({
    queryKey: queryKeys.guild.config(guildId),
    queryFn: () => api.guilds.config(guildId),
  });
}

/**
 * 自分のギルド内権限。API 側で Discord のメンバー情報を短時間キャッシュしているので、
 * Discord でロールを変えた直後に再取得しても反映まで少し遅れることがある
 * (待てないときは {@link useRefreshMyPermissions})
 */
export function useMyPermissionsQuery(guildId: string) {
  return useQuery({
    queryKey: queryKeys.guild.myPermissions(guildId),
    queryFn: () => api.guilds.myPermissions(guildId),
  });
}

/**
 * 権限を取り直す (#122)。Bot を招待し直したりロールを付けてもらった直後は API 側の
 * キャッシュが古いままなので、利用者の操作で捨てて取り直せるようにする。
 * 結果はそのままキャッシュに入れるので、連携チェックボックスの可否がその場で切り替わる
 */
export function useRefreshMyPermissions(guildId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.guilds.refreshMyPermissions(guildId),
    onSuccess: (permissions) => {
      queryClient.setQueryData<MyPermissions>(
        queryKeys.guild.myPermissions(guildId),
        permissions,
      );
    },
  });
}

/**
 * ギルド設定の更新 (管理権限が必要。なければ API が 403 を返す)。成功したらキャッシュを置き換える。
 * 管理コンソール側のキャッシュ (同じブラウザで開いていれば) も追従させる
 */
export function useUpdateGuildConfig(guildId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (restricted: boolean) =>
      api.guilds.updateConfig(guildId, restricted),
    onSuccess: (config) => {
      queryClient.setQueryData<GuildConfig>(
        queryKeys.guild.config(guildId),
        config,
      );
      syncAdminGuildConfig(queryClient, guildId, config);
    },
  });
}

/**
 * 予定を編集できるか。restricted モードでは管理権限を持つユーザーだけが編集できる (API 側でも強制される)。
 * 取得できていない間は閲覧のみ扱いにする
 */
export function canEditEvents(
  config: GuildConfig | undefined,
  permissions: MyPermissions | undefined,
): boolean {
  if (!config || !permissions) return false;
  return !config.restricted || permissions.can_manage_server;
}
