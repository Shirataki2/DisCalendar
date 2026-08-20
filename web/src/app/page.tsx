import Link from "next/link";

export default function Home() {
  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-10 p-8">
      <h1 className="font-mono text-5xl font-bold tracking-widest sm:text-6xl">
        DISCALENDAR
      </h1>
      <p className="max-w-xl text-center text-sm leading-7 text-neutral-300">
        DisCalendarはDiscord用のカレンダーアプリです。予定の作成から投稿まで面倒なコマンド操作はほとんど必要ありません。
        使い慣れたブラウザから、どこでも予定の追加や編集をすることができます。
      </p>
      <Link
        href="/dashboard"
        className="rounded-full bg-teal-700 px-10 py-3 text-sm font-semibold tracking-wide transition-colors hover:bg-teal-600"
      >
        カレンダー PoC を開く
      </Link>
    </main>
  );
}
