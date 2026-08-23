import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { AdminNav } from "@/components/admin-nav";
import { Logo } from "@/components/logo";
import { UserMenu } from "@/components/user-menu";
import { getAdminMe } from "@/lib/admin";
import { requireSession } from "@/lib/session";
import { ROUTES } from "@/lib/site";

// タイトルは各 page で付ける (title.template は同じセグメントの page には効かないため)
export const metadata: Metadata = {
  // 管理者専用ページなので検索エンジンには載せない
  robots: { index: false, follow: false },
};

/**
 * 管理コンソール (#33)。ログイン済みかつ api の `ADMIN_DISCORD_USER_IDS` に含まれるユーザーだけが開ける。
 * 判定は api の `GET /admin/me` (`AdminUser` extractor) に任せ、管理者でなければページの存在を見せないよう 404 にする。
 * 実際のデータ操作はすべて api の `/admin/*` が再度 `AdminUser` で拒否するので、ここは表示の入口にすぎない
 */
export default async function AdminLayout({ children }: LayoutProps<"/admin">) {
  const session = await requireSession();
  const admin = await getAdminMe();
  if (!admin) {
    notFound();
  }

  return (
    <div className="flex h-dvh flex-col">
      <header className="flex shrink-0 flex-wrap items-center gap-x-6 gap-y-2 border-b border-white/10 px-4 py-2">
        <Link
          href={ROUTES.admin}
          aria-label="管理コンソールのトップへ"
          className="flex items-center gap-2"
        >
          <Logo className="text-xl" />
          <span className="rounded bg-amber-500/20 px-1.5 py-0.5 text-[10px] font-semibold tracking-wide text-amber-300">
            ADMIN
          </span>
        </Link>
        <AdminNav />
        <div className="ml-auto flex items-center gap-4">
          <Link
            href={ROUTES.dashboard}
            className="text-xs text-neutral-400 transition-colors hover:text-white"
          >
            サーバー一覧へ
          </Link>
          <UserMenu
            name={session.user.name}
            image={session.user.image ?? null}
          />
        </div>
      </header>
      <div className="flex min-h-0 flex-1 flex-col">{children}</div>
    </div>
  );
}
