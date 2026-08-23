import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type { GuildConfig } from "@/lib/api/types";
import { syncAdminGuildConfig } from "./admin-cache";
import { queryKeys } from "./keys";

// 管理コンソール (#35)。初回の値は admin/guilds/[id]/page.tsx (RSC) が取得して hydrate する

/** ギルド詳細 (guilds + guild_config + event_settings + 予定数 + Bot の参加状況) */
export function useAdminGuildQuery(guildId: string) {
  return useQuery({
    queryKey: queryKeys.admin.guild(guildId),
    queryFn: () => api.admin.guilds.get(guildId),
  });
}

/**
 * restricted の切替 (api 側で監査ログに残る)。成功したらキャッシュ上の詳細を書き換える。
 * 同じギルドのダッシュボード (`/dashboard/[id]`) が使う設定キャッシュも同じ QueryClient にあるので、
 * そちらも置き換えて、戻ったときに編集可否が古いまま (staleTime 内) にならないようにする。
 * RSC が描画するギルド一覧 (`/admin/guilds`) は Router Cache を捨てて再取得させる
 */
export function useUpdateAdminGuildConfig(guildId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (restricted: boolean) =>
      api.admin.guilds.updateConfig(guildId, restricted),
    onSuccess: (config) => {
      syncAdminGuildConfig(queryClient, guildId, config);
      queryClient.setQueryData<GuildConfig>(
        queryKeys.guild.config(guildId),
        config,
      );
    },
  });
}
