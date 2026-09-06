import type { QueryClient } from "@tanstack/react-query";
import { revalidateAdminPages } from "@/app/admin/guilds/actions";
import type { AdminGuildDetail, GuildConfig } from "@/lib/api/types";
import { queryKeys } from "./keys";

// 通常画面 (/dashboard/[id]) と管理コンソール (/admin/guilds/*) は同じ guild_config / events を見ているので、
// どちらで書き込んでももう一方のキャッシュを追従させる。ここはその「管理側を最新にする」共通処理

/**
 * 管理コンソールの RSC ページ (ギルド一覧) の Router Cache を捨てる。
 * 書き込み本体 (api) は既に成功しているので、この再検証の失敗を mutation の失敗として返さない
 * (返すと保存済みなのに「失敗」と出て再試行 → 重複登録につながる)。失敗しても次の表示時に再取得されるだけ
 */
export function revalidateAdminPagesQuietly(): void {
  revalidateAdminPages().catch((error: unknown) => {
    console.warn("failed to revalidate admin pages", error);
  });
}

/**
 * 設定 (restricted・通知先チャンネル) が変わったとき、管理コンソールのギルド詳細キャッシュ (あれば) を追従させる。
 * 通知先の ID は管理権限を持つ人への応答にしか入らないが、この関数を呼ぶ mutation はどちらも
 * 管理権限が前提 (通常画面の保存は can_manage_server、管理コンソールは AdminUser) なので、
 * 設定済みなら ID が入っている
 */
export function syncAdminGuildConfig(
  queryClient: QueryClient,
  guildId: string,
  config: GuildConfig,
): void {
  queryClient.setQueryData<AdminGuildDetail>(
    queryKeys.admin.guild(guildId),
    // まだ管理画面を開いていなければ何もしない (undefined を返すと更新されない)
    (detail) =>
      detail
        ? {
            ...detail,
            restricted: config.restricted,
            channel_id: config.notification_channel_configured
              ? (config.notification_channel_id ?? detail.channel_id)
              : null,
          }
        : detail,
  );
  revalidateAdminPagesQuietly();
}
