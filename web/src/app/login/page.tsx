"use client";

import { Logo } from "@/components/logo";
import { authClient } from "@/lib/auth-client";

export default function LoginPage() {
  const signIn = () => {
    authClient.signIn.social({
      provider: "discord",
      callbackURL: "/dashboard",
    });
  };

  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-10 p-8">
      <h1>
        <Logo className="text-4xl" />
      </h1>
      <p className="text-sm text-neutral-300">
        Discordアカウントでログインして、サーバーのカレンダーを管理できます。
      </p>
      <button
        type="button"
        onClick={signIn}
        className="rounded-full bg-[#5865F2] px-10 py-3 text-sm font-semibold tracking-wide transition-colors hover:bg-[#4752c4]"
      >
        Discordでログイン
      </button>
    </main>
  );
}
