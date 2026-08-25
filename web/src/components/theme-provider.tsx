"use client";

import { usePathname } from "next/navigation";
import { ThemeProvider as NextThemesProvider } from "next-themes";
import type { ReactNode } from "react";
import { ROUTES } from "@/lib/site";

/**
 * テーマの保存先 (localStorage) のキー。
 * 旧実装 (Vuetify) も同じドメインの localStorage に "theme" で dark / light を保存していたが、
 * 旧実装のテーマ切替はレイアウトが `<v-app dark>` でダークを固定していて実際には効いていなかった。
 * その設定を引き継ぐと切替後に突然ライトになる利用者が出るので、キーを分けて引き継がない
 */
export const THEME_STORAGE_KEY = "discalendar-theme";

export const THEMES = ["dark", "light"] as const;

export type Theme = (typeof THEMES)[number];

export const DEFAULT_THEME: Theme = "dark";

/**
 * ダーク / ライトの切り替え (#58)。next-themes が localStorage の選択を読んで
 * <html> の class (Tailwind の dark バリアント) と data-color-scheme (FullCalendar の配色) を差し替える。
 * 反映は HTML の解析中に走るインラインスクリプトが行うので、読み込み時にちらつかない。
 *
 * ライトの配色を用意しているのはダッシュボードだけなので、それ以外のページ (LP・使い方・ログイン・
 * 管理コンソール) は forcedTheme でダークに固定する。選択自体は保存されたままなので、
 * ダッシュボードに戻れば選んだテーマで表示される
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const themeable = pathname?.startsWith(ROUTES.dashboard) ?? false;

  return (
    <NextThemesProvider
      attribute={["class", "data-color-scheme"]}
      themes={[...THEMES]}
      defaultTheme={DEFAULT_THEME}
      enableSystem={false}
      storageKey={THEME_STORAGE_KEY}
      disableTransitionOnChange
      forcedTheme={themeable ? undefined : DEFAULT_THEME}
    >
      {children}
    </NextThemesProvider>
  );
}
