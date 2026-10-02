"use client";

import Link from "next/link";
import { useLanguage } from "@/components/language-provider";
import { authClient } from "@/lib/auth-client";
import { dashboardReturnPath } from "@/lib/login-redirect";
import { ROUTES } from "@/lib/site";

const LINK_CLASS =
  "text-indigo-300 underline underline-offset-4 transition-colors hover:text-indigo-200";

export default function LoginPage() {
  const { t } = useLanguage();
  const signIn = () => {
    authClient.signIn.social({
      provider: "discord",
      callbackURL: dashboardReturnPath(
        new URLSearchParams(window.location.search).get("returnTo"),
      ),
    });
  };

  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-8 p-8">
      {/* ロゴはヘッダに出ているので、見出しはこの画面が何かを示す「ログイン」にする */}
      <div className="flex flex-col items-center gap-3 text-center">
        <h1 className="text-2xl font-bold tracking-wide">{t("ログイン")}</h1>
        <p className="text-sm text-neutral-300">
          {t(
            "Discordアカウントでログインして、サーバーのカレンダーを管理できます。",
          )}
        </p>
      </div>
      <div className="flex flex-col items-center gap-5">
        <button
          type="button"
          onClick={signIn}
          className="rounded-full bg-[#5865F2] px-10 py-3 text-sm font-semibold tracking-wide transition-colors hover:bg-[#4752c4]"
        >
          {t("Discordでログイン")}
        </button>
        <p className="max-w-xs text-center text-xs leading-6 text-neutral-400">
          {t("ログインすると、")}
          <Link href={ROUTES.tos} className={LINK_CLASS}>
            {t("利用規約")}
          </Link>

          {t("と")}
          <Link href={ROUTES.privacy} className={LINK_CLASS}>
            {t("プライバシーポリシー")}
          </Link>

          {t("に同意したものとみなします。")}
        </p>
      </div>
    </main>
  );
}
