import { SearchIcon } from "lucide-react";
import type { Metadata } from "next";
import { AdminPagination } from "@/components/admin-pagination";
import { AdminUserSessionsButton } from "@/components/admin-user-sessions";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { getAdminMe } from "@/lib/admin";
import { formatDateTime } from "@/lib/admin-format";
import { firstParam, lastPageOf, listHref, parsePage } from "@/lib/admin-list";
import { serverApi } from "@/lib/api/server";
import { ADMIN_USERS_MAX_PAGE, type AdminUserSummary } from "@/lib/api/types";
import { ROUTES } from "@/lib/site";

export const metadata: Metadata = {
  title: "ユーザー | 管理コンソール",
};

/**
 * ユーザーとセッションの管理 (#37)。検索とページングは URL (q / page) に持たせて RSC で取得し、
 * セッションの表示と強制ログアウトだけクライアント側で行う。
 * セッショントークンや Discord のトークンは api が返さないのでここにも出ない
 */
export default async function AdminUsersPage({
  searchParams,
}: PageProps<"/admin/users">) {
  const params = await searchParams;
  const q = firstParam(params.q).trim();
  const page = parsePage(firstParam(params.page), ADMIN_USERS_MAX_PAGE);
  // layout で管理者であることは確認済み (自分自身の行に印を付けるために使う)
  const [result, admin] = await Promise.all([
    serverApi.admin.users.list(q, page),
    getAdminMe(),
  ]);
  const lastPage = lastPageOf(result.total, result.page_size);

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-xl font-bold">ユーザー</h1>
          <p className="mt-1 text-sm text-neutral-400">
            Better Auth に登録されているユーザー。{result.total} 件
            {q && <> (「{q}」で検索)</>}
          </p>
        </div>
        <form
          action={ROUTES.adminUsers}
          method="get"
          className="flex items-center gap-2"
        >
          <Input
            type="search"
            name="q"
            defaultValue={q}
            placeholder="名前 / メール / user.id / Discord ID"
            aria-label="ユーザーを検索"
            className="w-72"
          />
          <Button type="submit" variant="outline" size="sm">
            <SearchIcon data-icon="inline-start" />
            検索
          </Button>
        </form>
      </div>

      <div className="overflow-x-auto rounded-lg border border-white/10">
        <table className="w-full text-sm">
          <thead className="bg-white/5 text-left text-xs text-neutral-400">
            <tr>
              <th className="px-3 py-2 font-medium">ユーザー</th>
              <th className="px-3 py-2 font-medium">Discord ID</th>
              <th className="px-3 py-2 font-medium">登録</th>
              <th className="px-3 py-2 font-medium">最終ログイン</th>
              <th className="px-3 py-2 text-right font-medium">セッション</th>
            </tr>
          </thead>
          <tbody>
            {result.items.length === 0 ? (
              <tr>
                <td
                  colSpan={5}
                  className="px-3 py-8 text-center text-neutral-500"
                >
                  該当するユーザーはいません
                </td>
              </tr>
            ) : (
              result.items.map((user) => (
                <UserRow
                  key={user.id}
                  user={user}
                  isSelf={user.id === admin?.user_id}
                />
              ))
            )}
          </tbody>
        </table>
      </div>

      <AdminPagination
        page={page}
        lastPage={lastPage}
        href={(next) => listHref(ROUTES.adminUsers, { q }, next)}
      />
    </main>
  );
}

function UserRow({
  user,
  isSelf,
}: {
  user: AdminUserSummary;
  isSelf: boolean;
}) {
  return (
    <tr className="border-t border-white/10 hover:bg-white/5">
      <td className="px-3 py-2">
        <div className="flex items-center gap-2">
          {user.image ? (
            // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
            <img src={user.image} alt="" className="h-6 w-6 rounded-full" />
          ) : (
            <span
              aria-hidden="true"
              className="inline-flex h-6 w-6 items-center justify-center rounded-full bg-white/10 text-xs"
            >
              {user.name.slice(0, 1)}
            </span>
          )}
          <div className="min-w-0">
            <p className="truncate font-medium">
              {user.name}
              {isSelf && (
                <span className="ml-2 rounded bg-amber-500/20 px-1.5 py-0.5 text-[10px] text-amber-300">
                  自分
                </span>
              )}
            </p>
            <p className="truncate text-xs text-neutral-400">{user.email}</p>
          </div>
        </div>
      </td>
      <td className="px-3 py-2 font-mono text-xs text-neutral-300">
        {user.discord_user_id ?? (
          <span className="text-neutral-500">未連携</span>
        )}
      </td>
      <td className="px-3 py-2 text-xs whitespace-nowrap text-neutral-300">
        {formatDateTime(user.created_at)}
      </td>
      <td className="px-3 py-2 text-xs whitespace-nowrap text-neutral-300">
        {user.last_session_at ? (
          formatDateTime(user.last_session_at)
        ) : (
          <span className="text-neutral-500">なし</span>
        )}
      </td>
      <td className="px-3 py-2">
        <div className="flex items-center justify-end gap-2">
          <span className="text-xs text-neutral-400 tabular-nums">
            有効 {user.active_sessions} / 全 {user.sessions}
          </span>
          <AdminUserSessionsButton user={user} isSelf={isSelf} />
        </div>
      </td>
    </tr>
  );
}
