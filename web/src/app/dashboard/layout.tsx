import Link from "next/link";
import { Logo } from "@/components/logo";
import { UserMenu } from "@/components/user-menu";
import { requireSession } from "@/lib/session";

export default async function DashboardLayout({
  children,
}: LayoutProps<"/dashboard">) {
  const session = await requireSession();

  return (
    <div className="flex h-dvh flex-col">
      <header className="flex shrink-0 items-center justify-between border-b border-white/10 px-4 py-2">
        <Link href="/dashboard" aria-label="サーバー選択へ">
          <Logo className="text-xl" />
        </Link>
        <UserMenu name={session.user.name} image={session.user.image ?? null} />
      </header>
      <div className="flex min-h-0 flex-1 flex-col">{children}</div>
    </div>
  );
}
