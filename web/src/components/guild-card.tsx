import type { ReactNode } from "react";

// サーバー選択画面 (/dashboard) のカード。Bot 参加済みの一覧は Server Component から、
// Bot を招待できるサーバーの一覧はクライアント側 (invite-guild-grid.tsx) から使うので、
// next/headers に依存する @/lib/discord は読まず、表示に要る値だけ props で受け取る

/** カードの見た目。invite (Bot 未参加) はグレースケールにして参加済みと区別する */
export function guildCardClassName(invite = false): string {
  // ライトでは surface (白) と地の色がほとんど変わらないので、枠でカードの範囲を見せる
  return `flex items-center gap-4 rounded-lg border border-border bg-surface p-4 transition-colors hover:bg-foreground/10 ${
    invite ? "grayscale hover:grayscale-0" : ""
  }`;
}

export function GuildGrid({ children }: { children: ReactNode }) {
  return (
    <ul className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {children}
    </ul>
  );
}

export function GuildCardBody({
  name,
  iconUrl,
  badge,
}: {
  name: string;
  iconUrl: string | null;
  /** カード右端の補足 (招待できるサーバーの「招待 ↗」など) */
  badge?: ReactNode;
}) {
  return (
    <>
      {iconUrl ? (
        // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
        <img src={iconUrl} alt="" className="h-12 w-12 rounded-full" />
      ) : (
        <span className="flex h-12 w-12 items-center justify-center rounded-full bg-foreground/10 text-lg font-bold">
          {name.slice(0, 1)}
        </span>
      )}
      <span className="flex-1 font-medium">{name}</span>
      {/* 「確認中…」は「招待 ↗」より長いので、縮めずにサーバー名側を折り返させる */}
      {badge && (
        <span className="shrink-0 text-xs text-muted-foreground">{badge}</span>
      )}
    </>
  );
}
