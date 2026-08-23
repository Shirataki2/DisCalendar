import Link from "next/link";
import { Logo } from "@/components/logo";
import { UserMenu } from "@/components/user-menu";
import { getAdminMe } from "@/lib/admin";
import { requireSession } from "@/lib/session";
import { ROUTES } from "@/lib/site";

export default async function DashboardLayout({
  children,
}: LayoutProps<"/dashboard">) {
  const session = await requireSession();
  // 管理者にだけ管理コンソールへの導線を出す。api に届かないときはリンクを出さないだけで画面は表示する
  const admin = await getAdminMe().catch(() => null);

  return (
    <div className="flex h-dvh flex-col">
      <header className="flex shrink-0 items-center justify-between border-b border-white/10 px-4 py-2">
        <Link href={ROUTES.dashboard} aria-label="サーバー選択へ">
          <Logo className="text-xl" />
        </Link>
        <div className="flex items-center gap-4">
          {admin && (
            <Link
              href={ROUTES.admin}
              className="text-xs text-amber-300 transition-colors hover:text-amber-200"
            >
              管理コンソール
            </Link>
          )}
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
