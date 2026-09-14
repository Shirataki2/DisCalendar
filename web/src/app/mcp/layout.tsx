import type { Metadata } from "next";
import { cookies, headers } from "next/headers";
import { DashboardFooter } from "@/components/dashboard-footer";
import { DashboardShell } from "@/components/dashboard-shell";
import { SiteFooter } from "@/components/site-footer";
import { SiteHeader } from "@/components/site-header";
import { UserMenu } from "@/components/user-menu";
import { getAdminMe } from "@/lib/admin";
import { auth } from "@/lib/auth";
import { isSidebarOpen, SIDEBAR_COOKIE } from "@/lib/dashboard-sidebar";

export const metadata: Metadata = {
  robots: { index: false, follow: false },
};

export default async function McpLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const session = await auth.api.getSession({ headers: await headers() });
  if (!session)
    return (
      <>
        <SiteHeader />
        {children}
        <SiteFooter />
      </>
    );
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
        <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
      </DashboardShell>
      <DashboardFooter />
    </div>
  );
}
