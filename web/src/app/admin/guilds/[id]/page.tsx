import {
  dehydrate,
  HydrationBoundary,
  QueryClient,
} from "@tanstack/react-query";
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { AdminGuildView } from "@/components/admin-guild-view";
import { ApiError } from "@/lib/api";
import { serverApi } from "@/lib/api/server";
import { queryKeys } from "@/lib/query/keys";

export const metadata: Metadata = {
  title: "ギルド詳細 | 管理コンソール",
};

/**
 * ギルドの詳細と予定のカレンダー (#35)。管理者はメンバーでないギルドも開ける。
 * 詳細は TanStack Query のキャッシュに入れて hydrate し、restricted の切替で即反映する
 */
export default async function AdminGuildPage({
  params,
}: PageProps<"/admin/guilds/[id]">) {
  const { id } = await params;
  if (!/^\d{1,20}$/.test(id)) {
    notFound();
  }

  const queryClient = new QueryClient();
  try {
    await queryClient.fetchQuery({
      queryKey: queryKeys.admin.guild(id),
      queryFn: () => serverApi.admin.guilds.get(id),
    });
  } catch (error) {
    // guilds テーブルに無いギルド (Bot が一度も参加していない) は 404
    if (error instanceof ApiError && error.status === 404) {
      notFound();
    }
    throw error;
  }

  return (
    <HydrationBoundary state={dehydrate(queryClient)}>
      <AdminGuildView guildId={id} />
    </HydrationBoundary>
  );
}
