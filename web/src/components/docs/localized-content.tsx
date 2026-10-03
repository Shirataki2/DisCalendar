"use client";

import type { ReactNode } from "react";
import { useLanguage } from "@/components/language-provider";

/** 両言語の本文はサーバーで描画し、選択した言語だけを表示・読み上げする。 */
export function LocalizedContent({
  ja,
  en,
}: {
  ja: ReactNode;
  en?: ReactNode;
}) {
  const { language } = useLanguage();
  return (
    <div lang={language === "en" && en ? "en" : "ja"}>
      {language === "en" && en ? en : ja}
    </div>
  );
}
