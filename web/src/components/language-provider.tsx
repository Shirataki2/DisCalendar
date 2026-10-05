"use client";

import { usePathname } from "next/navigation";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { FieldError } from "@/components/ui/field";
import { authClient } from "@/lib/auth-client";
import { DOC_PAGES } from "@/lib/docs";
import {
  LANGUAGE_STORAGE_KEY,
  type Language,
  type MessageValues,
  resolveLanguage,
  translate,
} from "@/lib/i18n";
import { english } from "@/lib/i18n/messages";
import { syncCurrentDeviceLanguage } from "@/lib/push";
import { SITE_NAME } from "@/lib/site";

const LanguageContext = createContext({
  language: "ja" as Language,
  setLanguage: (_language: Language) => {},
  t: (message: string, values?: MessageValues) =>
    translate("ja", message, values),
});

/** 公開ページも含めたブラウザの言語設定。SSR と hydration 直後は日本語で揃える。 */
export function LanguageProvider({ children }: { children: ReactNode }) {
  const pathname = usePathname();
  const [loaded, setLoaded] = useState(false);
  const [language, setCurrentLanguage] = useState<Language>("ja");
  useEffect(() => {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(LANGUAGE_STORAGE_KEY);
    } catch {
      // 保存が許可されないブラウザでも、このタブの言語切り替えは利用できる。
    }
    setCurrentLanguage(resolveLanguage(saved ?? navigator.language));
    setLoaded(true);
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
    const title =
      pathname === "/dashboard"
        ? "サーバー選択"
        : pathname === "/dashboard/all"
          ? "すべての予定"
          : /^\/dashboard\/\d+$/.test(pathname)
            ? "カレンダー"
            : /^\/dashboard\/\d+\/polls$/.test(pathname)
              ? "日程調整"
              : /^\/dashboard\/\d+\/polls\/\d+$/.test(pathname)
                ? "日程調整への回答"
                : pathname === "/tutorial"
                  ? "操作を試す"
                  : pathname === "/donation"
                    ? "支援"
                    : (DOC_PAGES.find(
                        (page) => pathname === `/docs/${page.slug}`,
                      )?.title ?? null);
    const syncTitle = () => {
      // URLだけでは404を判別できない。公開予定の同名タイトルは翻訳しない。
      const pageTitle = document.querySelector('[data-error-code="404"]')
        ? "ページが見つかりません"
        : title;
      const titleForms = pageTitle
        ? [
            `${pageTitle} | ${SITE_NAME}`,
            `${translate("en", pageTitle)} | ${SITE_NAME}`,
          ]
        : [];
      const localizedTitle = titleForms[language === "en" ? 1 : 0];
      if (
        titleForms.includes(document.title) &&
        document.title !== localizedTitle
      )
        document.title = localizedTitle;
      // 本文の lang は head に及ばないため、未翻訳のタイトルにも日本語を明示する。
      document
        .querySelector("title")
        ?.setAttribute(
          "lang",
          titleForms.includes(document.title) || document.title === SITE_NAME
            ? language
            : "ja",
        );
    };
    syncTitle();
    // 画面遷移後に Next が metadata を挿入しても、選択言語を維持する。
    const observer = new MutationObserver(syncTitle);
    observer.observe(document.head, {
      childList: true,
      characterData: true,
      subtree: true,
    });
    return () => observer.disconnect();
  }, [language, pathname]);

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
  return (
    <LanguageContext value={value}>
      {pathname !== "/tutorial" && (
        <PushLanguageSync language={language} loaded={loaded} />
      )}
      {children}
    </LanguageContext>
  );
}

/** 公開チュートリアルは認証・実データにアクセスせず、言語だけを切り替える。 */
function PushLanguageSync({
  language,
  loaded,
}: {
  language: Language;
  loaded: boolean;
}) {
  const { data: session } = authClient.useSession();
  const syncQueue = useRef(Promise.resolve());
  useEffect(() => {
    if (!loaded || !session?.user.id) return;
    let canceled = false;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let delay = 1000;
    const sync = () => {
      // 連続した切り替えでも、最後に選んだ言語が最後に保存されるよう直列化する。
      syncQueue.current = syncQueue.current
        .catch(() => {})
        .then(async () => {
          if (canceled) return;
          try {
            await syncCurrentDeviceLanguage(language);
            delay = 1000;
          } catch {
            if (!canceled) {
              clearTimeout(retry);
              retry = setTimeout(sync, delay);
              delay = Math.min(delay * 2, 60000);
            }
          }
        });
    };
    sync();
    window.addEventListener("online", sync);
    return () => {
      canceled = true;
      clearTimeout(retry);
      window.removeEventListener("online", sync);
    };
  }, [language, loaded, session?.user.id]);

  return null;
}

export function useLanguage(override?: Language) {
  const context = useContext(LanguageContext);
  return override
    ? {
        ...context,
        language: override,
        t: (message: string, values?: MessageValues) =>
          translate(override, message, values),
      }
    : context;
}

/** Server Component の案内も、ブラウザで選んだ言語に追従させる。 */
export function Message({
  message,
  values,
}: {
  message: string;
  values?: MessageValues;
}) {
  const { t, language } = useLanguage();
  const fallback =
    language === "en" &&
    /[ぁ-んァ-ヶ一-龠]/.test(message) &&
    !Object.hasOwn(english, message);
  return <span lang={fallback ? "ja" : language}>{t(message, values)}</span>;
}

/** 共有の検証ルールはそのまま使い、利用者に表示するエラーだけを翻訳する。 */
export function LocalizedFieldError({
  children,
  errors,
  language,
  ...props
}: React.ComponentProps<typeof FieldError> & { language?: Language }) {
  const { t } = useLanguage(language);
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
