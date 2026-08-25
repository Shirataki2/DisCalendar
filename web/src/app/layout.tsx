import type { Metadata, Viewport } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import { ServiceWorkerProvider } from "@/components/service-worker-provider";
import { ThemeProvider } from "@/components/theme-provider";
import { QueryProvider } from "@/lib/query/provider";
import { SITE_DESCRIPTION, SITE_NAME, SITE_URL, THEME_COLOR } from "@/lib/site";
import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

// favicon / アイコン / OGP 画像は app/ 直下のファイル規約 (favicon.ico, icon.png, apple-icon.png, opengraph-image.png) で、
// マニフェストは app/manifest.ts、Service Worker は app/sw.ts で出す。旧実装の siteconfig.js / @nuxtjs/pwa の設定に相当
export const metadata: Metadata = {
  // OGP など絶対 URL が必要な項目の基準。旧実装と同じく公開 URL を固定する (staging でも本番の URL になる)
  metadataBase: new URL(SITE_URL),
  title: {
    default: SITE_NAME,
    template: `%s | ${SITE_NAME}`,
  },
  description: SITE_DESCRIPTION,
  keywords: ["Discord", "Bot", "カレンダー", "予定管理", "スケジュール"],
  openGraph: {
    type: "website",
    locale: "ja_JP",
    siteName: SITE_NAME,
    title: SITE_NAME,
    description: SITE_DESCRIPTION,
    url: "/",
  },
  twitter: {
    card: "summary_large_image",
  },
};

export const viewport: Viewport = {
  themeColor: THEME_COLOR,
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  // class と data-color-scheme は ThemeProvider (next-themes) が選択に合わせて差し替える。
  // ここに書く dark は、差し替えるスクリプトが動く前と JavaScript が無効なときに使われる既定値
  return (
    <html
      lang="ja"
      data-color-scheme="dark"
      className={`dark ${geistSans.variable} ${geistMono.variable} h-full antialiased`}
      suppressHydrationWarning
    >
      <body className="min-h-full flex flex-col">
        <ThemeProvider>
          <ServiceWorkerProvider>
            <QueryProvider>{children}</QueryProvider>
          </ServiceWorkerProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
