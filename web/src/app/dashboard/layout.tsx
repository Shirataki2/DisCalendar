import { cookies } from "next/headers";
import { DashboardFooter } from "@/components/dashboard-footer";
import { DashboardShell } from "@/components/dashboard-shell";
import { UserMenu } from "@/components/user-menu";
import { getAdminMe } from "@/lib/admin";
import { isSidebarOpen, SIDEBAR_COOKIE } from "@/lib/dashboard-sidebar";
import { requireSession } from "@/lib/session";

/**
 * ダッシュボード (/dashboard, /dashboard/[id]) 共通のレイアウト。
 * 旧実装の layouts/authorized.vue と同じく「アプリバー + ナビゲーションドロワー + 固定フッタ」で囲み、
 * 本文はその間に収める (ページ全体はスクロールさせず、カレンダーが残りの高さいっぱいに広がる)
 */
export default async function DashboardLayout({
  children,
}: LayoutProps<"/dashboard">) {
  const session = await requireSession();
  // 管理者にだけ管理コンソールへの導線を出す。api に届かないときはリンクを出さないだけで画面は表示する
  const admin = await getAdminMe().catch(() => null);
  const cookieStore = await cookies();

  return (
    <div className="flex h-dvh flex-col">
      <DashboardShell
        admin={admin !== null}
        defaultSidebarOpen={isSidebarOpen(
          cookieStore.get(SIDEBAR_COOKIE)?.value,
        )}
        user={
          <UserMenu
            name={session.user.name}
            image={session.user.image ?? null}
          />
        }
      >
        {children}
      </DashboardShell>
      <DashboardFooter />
    </div>
  );
}
