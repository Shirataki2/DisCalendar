"use client";

import Link from "next/link";
import { authClient } from "@/lib/auth-client";
import { ROUTES } from "@/lib/site";

interface Props {
  className?: string;
}

/**
 * ログイン状態に応じて「ログイン」(/login) か「サーバー一覧」(/dashboard) を出すリンク。
 * LP は静的生成なので、セッションの有無はブラウザ側で Better Auth に問い合わせて切り替える
 * (旧実装の index.vue と同じ挙動)。確認できるまではログインへの導線を出しておく
 */
export function SessionLink({ className }: Props) {
  const { data: session } = authClient.useSession();
  if (session) {
    return (
      <Link href={ROUTES.dashboard} className={className}>
        サーバー一覧
      </Link>
    );
  }
  return (
    <Link href={ROUTES.login} className={className}>
      ログイン
    </Link>
  );
}
