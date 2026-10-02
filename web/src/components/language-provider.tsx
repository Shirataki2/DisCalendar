"use client";

import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { FieldError } from "@/components/ui/field";
import {
  LANGUAGE_STORAGE_KEY,
  type Language,
  type MessageValues,
  resolveLanguage,
  translate,
} from "@/lib/i18n";

const LanguageContext = createContext({
  language: "ja" as Language,
  setLanguage: (_language: Language) => {},
  t: (message: string, values?: MessageValues) =>
    translate("ja", message, values),
});

/** 公開ページも含めたブラウザの言語設定。SSR と hydration 直後は日本語で揃える。 */
export function LanguageProvider({ children }: { children: ReactNode }) {
  const [language, setCurrentLanguage] = useState<Language>("ja");
  useEffect(() => {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(LANGUAGE_STORAGE_KEY);
    } catch {
      // 保存が許可されないブラウザでも、このタブの言語切り替えは利用できる。
    }
    setCurrentLanguage(resolveLanguage(saved ?? navigator.language));
    const onStorage = (event: StorageEvent) => {
      if (event.storageArea !== localStorage) return;
      if (event.key === LANGUAGE_STORAGE_KEY || event.key === null) {
        setCurrentLanguage(
          resolveLanguage(event.newValue ?? navigator.language),
        );
      }
    };
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const setLanguage = useCallback((next: Language) => {
    setCurrentLanguage(next);
    try {
      localStorage.setItem(LANGUAGE_STORAGE_KEY, next);
    } catch {
      // 書き込めなくても現在のタブでは反映する。
    }
  }, []);
  const t = useCallback(
    (message: string, values?: MessageValues) =>
      translate(language, message, values),
    [language],
  );
  const value = useMemo(
    () => ({ language, setLanguage, t }),
    [language, setLanguage, t],
  );
  return <LanguageContext value={value}>{children}</LanguageContext>;
}

export function useLanguage() {
  return useContext(LanguageContext);
}

/** Server Component の案内も、ブラウザで選んだ言語に追従させる。 */
export function Message({
  message,
  values,
}: {
  message: string;
  values?: MessageValues;
}) {
  const { t } = useLanguage();
  return t(message, values);
}

/** 共有の検証ルールはそのまま使い、利用者に表示するエラーだけを翻訳する。 */
export function LocalizedFieldError({
  children,
  errors,
  ...props
}: React.ComponentProps<typeof FieldError>) {
  const { t } = useLanguage();
  return (
    <FieldError
      {...props}
      errors={errors?.map((error) =>
        error
          ? { ...error, message: error.message ? t(error.message) : undefined }
          : error,
      )}
    >
      {typeof children === "string" ? t(children) : children}
    </FieldError>
  );
}
