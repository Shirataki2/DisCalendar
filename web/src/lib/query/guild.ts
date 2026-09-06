import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@/lib/api";
import type {
  GuildConfig,
  GuildConfigInput,
  GuildFeed,
  MyPermissions,
} from "@/lib/api/types";
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
    // 実行中の通常の取得 (ウィンドウのフォーカス復帰など) を止める。放っておくと、
    // 取り直す前の値を読んだ応答があとから届いて、下の setQueryData を古い値で上書きしうる
    onMutate: () =>
      queryClient.cancelQueries({
        queryKey: queryKeys.guild.myPermissions(guildId),
      }),
    onError: () => {
      queryClient.setQueryData<MyPermissions>(
        queryKeys.guild.myPermissions(guildId),
        (previous) =>
          previous ? { ...previous, can_edit_events: false } : previous,
      );
    },
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
    mutationFn: (input: GuildConfigInput) =>
      api.guilds.updateConfig(guildId, input),
    // 進行中の取得 (ダイアログを開いたときの取り直しなど) を止める。放っておくと、保存前の値を読んだ
    // 応答があとから届いて、下の setQueryData を古い値で上書きしうる (useRefreshMyPermissions と同じ)
    onMutate: () =>
      queryClient.cancelQueries({ queryKey: queryKeys.guild.config(guildId) }),
    onSuccess: (config) => {
      queryClient.setQueryData<GuildConfig>(
        queryKeys.guild.config(guildId),
        config,
      );
      syncAdminGuildConfig(queryClient, guildId, config);
      return queryClient.invalidateQueries({
        queryKey: queryKeys.guild.myPermissions(guildId),
      });
    },
  });
}

/**
 * iCal フィードの発行状況 (#95)。RSC では事前取得せず、サーバー設定ダイアログを開いたときに取る。
 * 未発行なら data は null (undefined は未取得)
 */
export function useGuildFeedQuery(guildId: string) {
  return useQuery({
    queryKey: queryKeys.guild.feed(guildId),
    queryFn: () => api.guilds.feed(guildId),
  });
}

/**
 * フィードの発行・再発行 (管理権限が必要。なければ API が 403 を返す)。
 * 成功したら新しいトークンでキャッシュを置き換える (再発行後に古い URL が表示されたままにならないよう、
 * 実行中の取得はキャンセルする)
 */
export function useIssueGuildFeed(guildId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.guilds.issueFeed(guildId),
    onMutate: () =>
      queryClient.cancelQueries({ queryKey: queryKeys.guild.feed(guildId) }),
    onSuccess: (feed) => {
      queryClient.setQueryData<GuildFeed | null>(
        queryKeys.guild.feed(guildId),
        feed,
      );
    },
  });
}

/**
 * 通知先に選べるチャンネルの一覧 (#181)。サーバー設定ダイアログを開いたときに取る。
 * api 側で 1 分キャッシュしているので、Discord でチャンネルや権限を変えた直後は少し遅れて反映される
 */
export function useGuildChannelsQuery(guildId: string, enabled = true) {
  return useQuery({
    queryKey: queryKeys.guild.channels(guildId),
    queryFn: ({ signal }) => api.guilds.channels(guildId, signal),
    enabled,
    staleTime: 60_000,
  });
}

/** フィードの無効化 (管理権限が必要)。成功したらキャッシュを「未発行」にする */
export function useRevokeGuildFeed(guildId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => api.guilds.revokeFeed(guildId),
    onMutate: () =>
      queryClient.cancelQueries({ queryKey: queryKeys.guild.feed(guildId) }),
    onSuccess: () => {
      queryClient.setQueryData<GuildFeed | null>(
        queryKeys.guild.feed(guildId),
        null,
      );
    },
  });
}

/**
 * 予定を編集できるか。restricted モードでは管理権限または編集ロールを持つユーザーが編集できる (API 側でも強制される)。
 * 取得できていない間は閲覧のみ扱いにする
 */
export function canEditEvents(permissions: MyPermissions | undefined): boolean {
  return permissions?.can_edit_events ?? false;
}

/** 詳細を開いたときだけ必要な人を取得する。ギルドと ID の組でキャッシュする。 */
export function useMemberProfilesQuery(
  guildId: string,
  ids: string[],
  enabled: boolean,
) {
  const uniqueIds = [...new Set(ids)].sort();
  return useQuery({
    queryKey: queryKeys.guild.members(guildId, uniqueIds),
    queryFn: ({ signal }) => api.guilds.members(guildId, uniqueIds, signal),
    enabled: enabled && uniqueIds.length > 0,
    staleTime: 60_000,
    retry: false,
  });
}

/** 設定ダイアログを開いている間に取得する。Discord 側はギルドの既存キャッシュを共有する。 */
export function useGuildRolesQuery(guildId: string) {
  return useQuery({
    queryKey: queryKeys.guild.roles(guildId),
    queryFn: ({ signal }) => api.guilds.roles(guildId, signal),
    refetchOnMount: "always",
    retry: false,
  });
}
