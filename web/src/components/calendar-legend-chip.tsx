"use client";

import { cn } from "@/lib/utils";

/** 横断カレンダーと外部カレンダーで共用する表示切替の凡例。 */
export function CalendarLegendChip({
  name,
  color,
  shown,
  onClick,
  iconUrl,
  error,
  title,
}: {
  name: string;
  color: string;
  shown: boolean;
  onClick: () => void;
  iconUrl?: string | null;
  error?: string | null;
  title?: string;
}) {
  return (
    <button
      type="button"
      aria-pressed={shown}
      title={title}
      onClick={onClick}
      className={cn(
        "flex min-h-9 items-center gap-1.5 rounded-full border border-border px-2.5 py-1 text-xs transition-colors hover:bg-foreground/10 focus-visible:outline-2 focus-visible:outline-ring",
        !shown && "opacity-50",
      )}
    >
      <span
        aria-hidden
        className="size-3 shrink-0 rounded-full border-2"
        style={
          shown
            ? { backgroundColor: color, borderColor: color }
            : { borderColor: color }
        }
      />
      {iconUrl && (
        // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
        <img src={iconUrl} alt="" className="size-4 shrink-0 rounded-full" />
      )}
      <span className="max-w-40 truncate">{name}</span>
      {error && <span className="text-destructive">{error}</span>}
    </button>
  );
}
