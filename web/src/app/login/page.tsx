"use client";

import Link from "next/link";
import { authClient } from "@/lib/auth-client";
import { ROUTES } from "@/lib/site";

const LINK_CLASS =
  "text-indigo-300 underline underline-offset-4 transition-colors hover:text-indigo-200";

export default function LoginPage() {
  const signIn = () => {
    authClient.signIn.social({
      provider: "discord",
      callbackURL: "/dashboard",
    });
  };

  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-8 p-8">
      {/* ロゴはヘッダに出ているので、見出しはこの画面が何かを示す「ログイン」にする */}
      <div className="flex flex-col items-center gap-3 text-center">
        <h1 className="text-2xl font-bold tracking-wide">ログイン</h1>
        <p className="text-sm text-neutral-300">
          Discordアカウントでログインして、サーバーのカレンダーを管理できます。
        </p>
      </div>
      <div className="flex flex-col items-center gap-5">
        <button
          type="button"
          onClick={signIn}
          className="rounded-full bg-[#5865F2] px-10 py-3 text-sm font-semibold tracking-wide transition-colors hover:bg-[#4752c4]"
        >
          Discordでログイン
        </button>
        <p className="max-w-xs text-center text-xs leading-6 text-neutral-400">
          ログインすると、
          <Link href={ROUTES.tos} className={LINK_CLASS}>
            利用規約
          </Link>
          と
          <Link href={ROUTES.privacy} className={LINK_CLASS}>
            プライバシーポリシー
          </Link>
          に同意したものとみなします。
        </p>
      </div>
    </main>
  );
}
