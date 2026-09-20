"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BellRingIcon, CheckIcon, RotateCcwIcon } from "lucide-react";
import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import { EventCalendar } from "@/components/event-calendar";
import { Button } from "@/components/ui/button";
import type { ApiEvent } from "@/lib/api/types";
import {
  describeEventRange,
  describeNotification,
} from "@/lib/calendar-events";
import { DEFAULT_CALENDAR_SETTINGS } from "@/lib/calendar-settings";
import { ROUTES } from "@/lib/site";
import {
  createTutorialEventsSource,
  TUTORIAL_GUILD_ID,
} from "@/lib/tutorial-events";

const STEPS = [
  {
    title: "まずはカレンダーを見てみよう",
    text: "ここは練習用のサーバーです。月・週・日の表示を切り替えたり、予定を開いたりしてみましょう。準備ができたら次へ進みます。",
  },
  {
    title: "自分の予定を作ってみよう",
    text: "「新規作成」からタイトルと日時を入力し、「作成」を押してください。通知を何分前に届けるかも設定できます。",
  },
  {
    title: "作った予定を編集してみよう",
    text: "予定を開いて「編集」を選び、タイトルや説明、通知のタイミングを変えて「保存」を押してください。",
  },
  {
    title: "Discordに届く通知を見てみよう",
    text: "今の予定で通知の見本を作りました。事前通知と開始時刻の通知を切り替えて確認できます。実際の送信は行いません。",
  },
  {
    title: "予定を削除してみよう",
    text: "予定を開いて「削除」を選び、確認画面でも「削除」を押してください。実際のサーバーではメンバー全員のカレンダーから消えます。",
  },
  {
    title: "これで基本の操作は完了です",
    text: "作成・編集・通知・削除を体験できました。このまま自由に試したり、自分のサーバーで使い始めたりできます。",
  },
] as const;

interface Progress {
  step: number | null;
  unlocked: number;
  event: ApiEvent | null;
}

export function Tutorial() {
  const [ready, setReady] = useState(false);
  const [attempt, setAttempt] = useState(0);
  // サンプルの日付はブラウザで決め、SSR との日付差を避ける。
  useEffect(() => setReady(true), []);
  if (!ready) return <p role="status">練習用カレンダーを準備しています…</p>;
  return (
    <TutorialSession
      key={attempt}
      onRestart={() => setAttempt((value) => value + 1)}
    />
  );
}

function TutorialSession({ onRestart }: { onRestart: () => void }) {
  const [progress, setProgress] = useState<Progress>({
    step: 0,
    unlocked: 0,
    event: null,
  });
  // 通常画面のキャッシュに触れず、再訪問・やり直しで確実に捨てる。
  const [queryClient] = useState(() => new QueryClient());
  const [source] = useState(() =>
    createTutorialEventsSource((action, event) => {
      setProgress((previous) => {
        const { step } = previous;
        const advances =
          (step === 1 && action === "create") ||
          (step === 2 && action === "update") ||
          (step === 4 && action === "remove");
        const removedTarget =
          action === "remove" && previous.event?.id === event.id;
        // 案内の途中で対象を消しても、作り直して続けられる。
        const nextStep = advances
          ? step + 1
          : removedTarget && (step === 2 || step === 3)
            ? 1
            : step;
        return {
          step: nextStep,
          unlocked:
            nextStep === 1 && removedTarget
              ? 1
              : Math.max(previous.unlocked, nextStep ?? 0),
          event:
            action === "remove"
              ? removedTarget
                ? null
                : previous.event
              : event,
        };
      });
    }),
  );
  const goTo = (step: number) =>
    setProgress((previous) => ({
      ...previous,
      step,
      unlocked: Math.max(previous.unlocked, step),
    }));
  const endGuide = () =>
    setProgress((previous) => ({ ...previous, step: null }));
  const guideProps = { progress, goTo, endGuide, onRestart };

  return (
    <QueryClientProvider client={queryClient}>
      <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <span
            aria-hidden
            className="flex size-10 items-center justify-center rounded-full bg-indigo-500/20 font-bold text-indigo-200"
          >
            練
          </span>
          <div>
            <p className="font-semibold">練習用サーバー</p>
            <p className="text-xs text-muted-foreground">
              予定の日時は日本時間です
            </p>
          </div>
        </div>
        <Button variant="outline" onClick={onRestart}>
          <RotateCcwIcon />
          最初からやり直す
        </Button>
      </div>
      <div className="flex flex-1 flex-col [&_.calendar-shell]:h-[max(32rem,60dvh)] [&_.calendar-shell]:flex-none">
        <EventCalendar
          guildName={"練習用サーバー"}
          guildId={TUTORIAL_GUILD_ID}
          canEdit
          eventsSource={source}
          settingsOverride={DEFAULT_CALENDAR_SETTINGS}
          guide={{
            target:
              progress.step === 1
                ? "create"
                : progress.step === 2 || progress.step === 4
                  ? progress.event?.id
                  : undefined,
            date: progress.event?.start_at,
            content: (
              <TutorialGuide key={progress.step ?? "free"} {...guideProps} />
            ),
            inlineContent:
              progress.step === null ? (
                <p className="text-xs text-indigo-200">
                  練習用・Discordには送信されません
                </p>
              ) : (
                <TutorialGuide {...guideProps} compact />
              ),
          }}
        />
      </div>
    </QueryClientProvider>
  );
}

function TutorialGuide({
  progress,
  goTo,
  endGuide,
  onRestart,
  compact = false,
}: {
  progress: Progress;
  goTo: (step: number) => void;
  endGuide: () => void;
  onRestart: () => void;
  compact?: boolean;
}) {
  const { step, unlocked, event } = progress;
  const heading = useRef<HTMLHeadingElement>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  useEffect(() => {
    // フォームの初期フォーカスはタイトル入力欄に任せる。
    if (!compact) heading.current?.focus({ preventScroll: true });
  }, [compact]);
  const current = step === null ? null : STEPS[step];
  const canContinue =
    (step === 0 || step === 3 || (step !== null && step < unlocked)) &&
    !(compact && step === 3);

  return (
    <section
      aria-label={compact ? "操作のヒント" : "チュートリアル"}
      className={`relative rounded-xl border border-indigo-400/40 bg-indigo-950/80 p-4 text-indigo-50 ${compact ? "text-sm" : "lg:before:absolute lg:before:top-8 lg:before:-left-2 lg:before:size-4 lg:before:rotate-45 lg:before:border-b lg:before:border-l lg:before:border-indigo-400/40 lg:before:bg-indigo-950"}`}
    >
      <div role="status" aria-atomic="true">
        <p className="mb-2 text-xs font-medium text-indigo-200">
          {step === null
            ? "自由に試せます"
            : `ステップ ${step + 1} / ${STEPS.length}`}
        </p>
        <h2
          ref={heading}
          tabIndex={-1}
          className="font-semibold leading-relaxed outline-none"
        >
          {current?.title ?? "自分のペースで試してみよう"}
        </h2>
      </div>
      {compact ? (
        <>
          <p className="mt-2 text-sm leading-6 text-indigo-100">
            {current?.text ??
              "予定の作成・編集・削除を自由に試せます。作成・編集した予定の通知もここで確認できます。"}
          </p>
          <p className="mt-2 text-xs text-indigo-200">
            練習用・Discordには送信されません
            {step === 3 && "。この画面を閉じると通知の見本を確認できます。"}
          </p>
        </>
      ) : (
        <div className="mt-2">
          <button
            type="button"
            aria-controls="tutorial-guide-details"
            aria-expanded={detailsOpen}
            className="min-h-11 rounded-md text-sm font-medium text-indigo-100 underline underline-offset-4 lg:hidden"
            onClick={() => setDetailsOpen((open) => !open)}
          >
            {detailsOpen ? "詳しい説明を閉じる" : "詳しい説明"}
            {!detailsOpen && (step === 3 || step === null)
              ? "・通知プレビュー"
              : ""}
          </button>
          <div
            id="tutorial-guide-details"
            className={detailsOpen ? "block" : "hidden lg:block"}
          >
            <p className="mt-2 text-sm leading-6 text-indigo-100">
              {current?.text ??
                "予定の作成・編集・削除を自由に試せます。作成・編集した予定の通知もここで確認できます。"}
            </p>
            {(step === 3 || step === null) &&
              (event ? (
                <NotificationPreview key={event.id} event={event} />
              ) : (
                <p className="mt-4 text-sm text-indigo-200">
                  予定を作成すると、通知の見本を確認できます。
                </p>
              ))}
            {(step === 5 || step === null) && <TutorialNextSteps />}
            <p className="mt-4 border-t border-indigo-300/20 pt-3 text-xs leading-5 text-indigo-200">
              練習した予定は保存されません。ページを開き直すと最初の状態に戻ります。
            </p>
          </div>
        </div>
      )}
      <div className="mt-4 flex flex-wrap items-center gap-2">
        {step !== null && step > 0 && (
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => goTo(step - 1)}
          >
            前の説明へ
          </Button>
        )}
        {step !== null && step < 5 && (
          <Button
            type="button"
            size="sm"
            disabled={!canContinue}
            onClick={() => goTo(step + 1)}
          >
            {step === 3 ? "通知を確認した" : "次へ"}
          </Button>
        )}
        {step !== null && (
          <Button type="button" size="sm" variant="ghost" onClick={endGuide}>
            {step === 5 ? "自由に試す" : "案内を終了"}
          </Button>
        )}
        {step === null && (
          <Button type="button" size="sm" variant="outline" onClick={onRestart}>
            案内を最初から見る
          </Button>
        )}
      </div>
      {!compact && step !== null && !canContinue && step < 5 && (
        <p className="mt-3 text-xs leading-5 text-indigo-200">
          操作が完了すると、自動で次のステップへ進みます。
        </p>
      )}
    </section>
  );
}

function NotificationPreview({ event }: { event: ApiEvent }) {
  const [selected, setSelected] = useState("start");
  const selectedNotification = event.notifications[Number(selected)];
  const notification = selected === "start" ? undefined : selectedNotification;
  const introduction = notification
    ? `${describeNotification(notification).replace(/前$/, "後")}に以下の予定が開催されます`
    : "以下の予定が開催されます";
  return (
    <div className="mt-4 space-y-3">
      <label
        className="block text-xs font-medium"
        htmlFor="tutorial-notification"
      >
        通知プレビュー
      </label>
      <select
        id="tutorial-notification"
        className="h-9 w-full rounded-md border border-white/20 bg-surface px-2 text-sm text-foreground"
        value={notification ? selected : "start"}
        onChange={(change) => setSelected(change.target.value)}
      >
        <option value="start">開始時刻の通知</option>
        {event.notifications.map((item, index) => (
          // 同じ設定を複数追加できる既存フォームに合わせ、選択肢は入力順にする。
          // biome-ignore lint/suspicious/noArrayIndexKey: 通知設定は入力順で識別する
          <option key={index} value={index}>
            {describeNotification(item)}の通知
          </option>
        ))}
      </select>
      <figure
        className="rounded-lg bg-[#313338] p-3 text-[#f2f3f5]"
        aria-label="Discord通知の見本"
      >
        <p className="mb-3 flex items-center gap-2 text-xs">
          <BellRingIcon className="size-4" aria-hidden />
          DisCalendar{" "}
          <span className="rounded bg-[#5865F2] px-1.5 py-0.5 text-[10px]">
            Bot
          </span>
        </p>
        <div
          className="space-y-2 rounded border-l-4 bg-[#2b2d31] p-3 break-words"
          style={{ borderColor: event.color }}
        >
          <p className="text-xs">{introduction}</p>
          <p className="font-semibold">{event.name}</p>
          {event.description && (
            <p className="whitespace-pre-wrap text-sm">{event.description}</p>
          )}
          <p className="text-xs font-semibold">日時</p>
          <p className="text-xs">{describeEventRange(event)}</p>
        </div>
      </figure>
      <p className="text-xs leading-5 text-indigo-200">
        本番ではサーバー設定で通知先チャンネルを選びます。開始時刻の通知もサーバー設定で変更できます。
      </p>
    </div>
  );
}

function TutorialNextSteps() {
  return (
    <div className="mt-4 space-y-3 text-sm">
      <p className="flex items-center gap-2 font-medium">
        <CheckIcon className="size-4" aria-hidden />
        自分のサーバーで使うには
      </p>
      <div className="flex flex-col gap-2">
        <a
          className="rounded-md bg-[#5865F2] px-3 py-2 text-center font-medium text-white hover:bg-[#4752c4]"
          href={ROUTES.invite}
          target="_blank"
          rel="noreferrer"
        >
          Botを招待する
        </a>
        <Link
          prefetch={false}
          className="rounded-md border border-indigo-300/30 px-3 py-2 text-center hover:bg-indigo-400/10"
          href={ROUTES.dashboard}
        >
          サーバー一覧を開く
        </Link>
        <Link
          prefetch={false}
          className="py-1 underline underline-offset-4"
          href={ROUTES.docs}
        >
          導入手順を見る
        </Link>
      </div>
      <p className="text-xs leading-5 text-indigo-200">
        通知先は Web
        のサーバー設定から選べます。Botの招待・通知先の設定や、編集権限の付与ができない場合は、サーバーの管理者に相談してください。
      </p>
    </div>
  );
}
