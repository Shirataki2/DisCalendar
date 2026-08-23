import localFont from "next/font/local";
import { cn } from "@/lib/utils";

// ロゴ用フォント (旧実装と同じ Uni Sans Heavy。Fontfabric の無償ウェイトで商用利用可)。
// ロゴの数文字にしか使わないので preload せず、表示までの間はフォールバックを出す
const logoFont = localFont({
  src: "../assets/fonts/UniSansHeavy.otf",
  weight: "900",
  display: "swap",
  preload: false,
  variable: "--font-logo",
});

/** "DisCalendar" のワードマーク。ヘッダ・LP・ログイン画面で共通に使う */
export function Logo({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        logoFont.className,
        "inline-block leading-none tracking-wide",
        className,
      )}
    >
      DisCalendar
    </span>
  );
}
