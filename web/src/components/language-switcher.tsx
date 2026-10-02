"use client";

import { useLanguage } from "@/components/language-provider";

export function LanguageSwitcher() {
  const { language, setLanguage } = useLanguage();
  return (
    <select
      aria-label="Language / 言語"
      value={language}
      onChange={(event) =>
        setLanguage(event.target.value === "en" ? "en" : "ja")
      }
      className="h-11 max-w-28 shrink-0 rounded-md border border-border bg-background px-2 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
    >
      <option value="ja" lang="ja">
        日本語
      </option>
      <option value="en" lang="en">
        English
      </option>
    </select>
  );
}
