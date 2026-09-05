import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";
import { getSharedEvent } from "@/lib/api/public-share";
import { describeEventRange } from "@/lib/calendar-events";
import { SITE_URL } from "@/lib/site";

export const dynamic = "force-dynamic";
type Props = { params: Promise<{ token: string }> };

export async function generateMetadata({ params }: Props): Promise<Metadata> {
  const { token } = await params;
  const event = await getSharedEvent(token);
  if (!event) notFound();
  const description = `${describeEventRange(event)}${event.is_all_day ? " (終日)" : " (日本時間)"} · ${event.guild_name}`;
  return {
    metadataBase: new URL(SITE_URL),
    title: event.name,
    description,
    robots: { index: false, follow: false },
    referrer: "no-referrer",
    openGraph: {
      title: event.name,
      description,
      url: `/share/${token}`,
      type: "website",
    },
    twitter: { card: "summary_large_image", title: event.name, description },
  };
}

export default async function SharePage({ params }: Props) {
  const event = await getSharedEvent((await params).token);
  if (!event) notFound();
  return (
    <main className="mx-auto w-full max-w-2xl px-5 py-12 sm:py-20">
      <Link href="/" className="text-sm font-semibold text-muted-foreground">
        DisCalendar
      </Link>
      <article className="mt-8 space-y-6 rounded-2xl border bg-card p-6 sm:p-10">
        <p className="text-sm text-muted-foreground break-words">
          {event.guild_name}
        </p>
        <h1 className="text-3xl font-bold break-words">{event.name}</h1>
        <p>
          {describeEventRange(event)}{" "}
          <span className="text-muted-foreground">
            {event.is_all_day ? "終日" : "(日本時間)"}
          </span>
        </p>
        {event.description && (
          <p className="whitespace-pre-wrap break-words leading-relaxed">
            {event.description}
          </p>
        )}
        <div className="border-t pt-6">
          <Link
            href={`/dashboard/${event.guild_id}`}
            className="text-primary underline underline-offset-4"
          >
            サーバーのカレンダーを開く
          </Link>
          <p className="mt-2 text-xs text-muted-foreground">
            カレンダーの閲覧にはログインとサーバーへの参加が必要です。
          </p>
        </div>
      </article>
    </main>
  );
}
