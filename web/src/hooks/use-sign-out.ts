"use client";

import { useRouter } from "next/navigation";
import { useCallback } from "react";
import { authClient } from "@/lib/auth-client";
import { ROUTES } from "@/lib/site";

/** ログアウトしてログイン画面へ戻す (アカウントメニューとナビゲーションドロワーで共用) */
export function useSignOut() {
  const router = useRouter();
  return useCallback(async () => {
    await authClient.signOut();
    router.push(ROUTES.login);
  }, [router]);
}
