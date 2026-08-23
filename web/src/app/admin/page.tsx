import type { Metadata } from "next";
import { getAdminMe } from "@/lib/admin";

export const metadata: Metadata = {
  title: "管理コンソール",
};

// 今後の画面 (子 Issue)。トップはそれらへの案内を置いておき、#37 で稼働状況の概要に差し替える
const UPCOMING = [
  {
    title: "ギルド・予定",
    description: "全ギルドの一覧・検索と、ギルドごとの予定の閲覧・編集・削除",
    issue: 35,
  },
  {
    title: "SQL コンソール",
    description: "読み取り専用 SQL の実行と、定型の書き込み操作",
    issue: 36,
  },
  {
    title: "稼働状況・ユーザー",
    description:
      "件数・DB / マイグレーション状況、ユーザーとセッションの管理、監査ログの閲覧",
    issue: 37,
  },
] as const;

export default async function AdminPage() {
  // layout で管理者であることは確認済み (非管理者は 404)
  const admin = await getAdminMe();

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <h1 className="mb-2 text-xl font-bold">管理コンソール</h1>
      <p className="mb-8 text-sm text-neutral-400">
        {admin?.name} (Discord ID: {admin?.discord_user_id})
        として管理者権限でログインしています。ここでの操作はすべて監査ログに記録されます。
      </p>
      <section aria-labelledby="upcoming-heading">
        <h2 id="upcoming-heading" className="mb-3 text-sm font-semibold">
          準備中の画面
        </h2>
        <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {UPCOMING.map((item) => (
            <li
              key={item.issue}
              className="rounded-lg border border-white/10 bg-white/5 p-4"
            >
              <p className="font-medium">{item.title}</p>
              <p className="mt-1 text-sm text-neutral-400">
                {item.description}
              </p>
              <p className="mt-2 text-xs text-neutral-500">
                Issue #{item.issue}
              </p>
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}
