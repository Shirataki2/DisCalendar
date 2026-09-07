"use client";

import { useRouter } from "next/navigation";
import { useCallback } from "react";
import { authClient } from "@/lib/auth-client";
import { unsubscribeCurrentDevice } from "@/lib/push";
import { ROUTES } from "@/lib/site";

/** ログアウトしてトップページへ戻す (アカウントメニューとナビゲーションドロワーで共用) */
export function useSignOut() {
  const router = useRouter();
  return useCallback(async () => {
    try {
      await unsubscribeCurrentDevice();
    } catch {
      window.alert(
        "通知の解除に失敗しました。接続を確認してログアウトをやり直してください。",
      );
      return;
    }
    await authClient.signOut();
    router.push(ROUTES.home);
  }, [router]);
}
