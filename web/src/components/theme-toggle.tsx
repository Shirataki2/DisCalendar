"use client";

import { MoonIcon, SunIcon } from "lucide-react";
import { useTheme } from "next-themes";
import { cn } from "@/lib/utils";

/**
 * ダークとライトを入れ替える関数を返す (旧実装の AccountMenu.vue / NavDrawer.vue の「テーマ変更」相当)。
 * 現在のテーマは読まずに更新関数で反転させる。next-themes の theme はサーバー側では既定値、
 * ブラウザでは localStorage の値になるため、描画に使うと hydration がずれる
 */
export function useThemeToggle(): () => void {
  const { setTheme } = useTheme();

  return () => setTheme((current) => (current === "light" ? "dark" : "light"));
}

/**
 * 切替ボタンの中身 (アイコンとラベル)。「今のテーマ」ではなく「切り替え先」を出す。
 * どちらを出すかは state ではなく CSS (dark バリアント) で決めて、上と同じ理由で hydration のずれを避ける
 */
export function ThemeToggleContent({
  iconClassName,
  labelClassName,
}: {
  iconClassName?: string;
  labelClassName?: string;
}) {
  return (
    <>
      <SunIcon className={cn("hidden dark:block", iconClassName)} aria-hidden />
      <MoonIcon className={cn("dark:hidden", iconClassName)} aria-hidden />
      <span className={labelClassName}>
        <span className="hidden dark:inline">ライトテーマに切り替え</span>
        <span className="dark:hidden">ダークテーマに切り替え</span>
      </span>
    </>
  );
}
