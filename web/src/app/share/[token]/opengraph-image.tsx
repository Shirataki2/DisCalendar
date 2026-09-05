import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { ImageResponse } from "next/og";
import { getSharedEvent } from "@/lib/api/public-share";
import { describeEventRange } from "@/lib/calendar-events";

export const dynamic = "force-dynamic";
export const alt = "DisCalendar の共有予定";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default async function Image({
  params,
}: {
  params: Promise<{ token: string }>;
}) {
  const event = await getSharedEvent((await params).token);
  if (!event)
    return new Response(null, {
      status: 404,
      headers: { "Cache-Control": "no-store" },
    });
  // 予定の文字列を外部フォントサービスへ送らず、日本語をオフラインでも描画する。
  const font = await readFile(
    join(process.cwd(), "src/assets/fonts/NotoSansJP-Medium.woff"),
  );
  return new ImageResponse(
    <div
      lang="ja-JP"
      style={{
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        width: "100%",
        height: "100%",
        padding: 64,
        background: "#171923",
        color: "#ffffff",
        borderTop: "14px solid #5865F2",
      }}
    >
      <div style={{ display: "flex", fontSize: 30, color: "#b7baff" }}>
        DisCalendar · 共有予定
      </div>
      <div
        style={{
          display: "flex",
          fontSize: 60,
          fontWeight: 700,
          lineHeight: 1.25,
        }}
      >
        {event.name}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
        <div style={{ display: "flex", fontSize: 30 }}>
          {describeEventRange(event)}
          {event.is_all_day ? " · 終日" : " · 日本時間"}
        </div>
        <div style={{ display: "flex", fontSize: 28, color: "#c3c5d1" }}>
          {Array.from(event.guild_name).slice(0, 50).join("")}
        </div>
      </div>
    </div>,
    {
      ...size,
      fonts: [
        { name: "Noto Sans JP", data: font, weight: 500, style: "normal" },
      ],
      headers: { "Cache-Control": "no-store" },
    },
  );
}
