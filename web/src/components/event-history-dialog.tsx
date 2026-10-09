"use client";

import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { EventHistoryEntry, MemberProfile } from "@/lib/api/types";
import {
  describeEventRange,
  describeNotification,
} from "@/lib/calendar-events";
import {
  diffEventSnapshots,
  type EventHistoryChange,
  formatHistoryTime,
  historyActorIds,
} from "@/lib/event-history";
import type { Language } from "@/lib/i18n";
import { useEventHistoryQuery } from "@/lib/query/events";
import { useMemberProfilesInChunks } from "@/lib/query/guild";

interface Props {
  guildId: string;
  eventId: number;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type Translate = ReturnType<typeof useLanguage>["t"];

const ACTION_LABELS = {
  create: "予定を作成しました",
  update: "予定を変更しました",
  delete: "予定を削除しました",
  restore: "予定を復元しました",
} as const;

const SOURCE_LABELS = {
  web: null,
  bot: "Bot から",
  admin: "管理コンソールから",
  mcp: "AI アシスタントから",
} as const;

/** 予定の変更履歴 (#165)。新しい順に、更新は変わった項目だけを並べる */
export function EventHistoryDialog({
  guildId,
  eventId,
  open,
  onOpenChange,
}: Props) {
  const { t, language } = useLanguage();
  const history = useEventHistoryQuery(guildId, eventId, open);
  const entries = history.data ?? [];
  const profiles = useMemberProfilesInChunks(
    guildId,
    historyActorIds(entries),
    open,
  );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>{t("変更履歴")}</DialogTitle>
          <DialogDescription>
            {t(
              "この予定を誰がいつどう変えたかを新しい順に表示します。直近 50 件まで表示し、180 日を過ぎた履歴は消えます。",
            )}
          </DialogDescription>
        </DialogHeader>
        {history.isPending ? (
          <p className="text-sm text-muted-foreground">{t("読み込み中…")}</p>
        ) : history.isError ? (
          <div className="flex flex-col items-start gap-2 text-sm">
            <p className="text-destructive">
              {t("変更履歴を読み込めませんでした")}
            </p>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void history.refetch()}
            >
              {t("再試行")}
            </Button>
          </div>
        ) : entries.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            {t("この予定の変更履歴はまだありません")}
          </p>
        ) : (
          <ol className="flex flex-col divide-y" data-testid="event-history">
            {entries.map((entry) => (
              <HistoryItem
                key={entry.id}
                entry={entry}
                profiles={profiles.data}
                // 取得し終えても見つからない (失敗・応答に含まれない) ときは「読み込み中」のままにしない
                profilesSettled={!profiles.isPending}
                t={t}
                language={language}
              />
            ))}
          </ol>
        )}
      </DialogContent>
    </Dialog>
  );
}

function HistoryItem({
  entry,
  profiles,
  profilesSettled,
  t,
  language,
}: {
  entry: EventHistoryEntry;
  profiles: MemberProfile[] | undefined;
  profilesSettled: boolean;
  t: Translate;
  language: Language;
}) {
  const changes =
    entry.action === "update" && entry.before && entry.after
      ? diffEventSnapshots(entry.before, entry.after)
      : [];
  const source = SOURCE_LABELS[entry.source] ?? null;
  return (
    <li className="flex flex-col gap-1 py-2.5 text-sm first:pt-0 last:pb-0">
      <div className="flex flex-wrap items-center gap-x-1.5 gap-y-0.5">
        <Actor
          entry={entry}
          profiles={profiles}
          profilesSettled={profilesSettled}
          t={t}
        />
        <span className="text-muted-foreground">
          {t(ACTION_LABELS[entry.action] ?? "予定を変更しました")}
        </span>
      </div>
      <div className="flex flex-wrap gap-x-2 text-xs text-muted-foreground">
        <time dateTime={entry.created_at}>
          {formatHistoryTime(entry.created_at)}
        </time>
        {source && <span>{t(source)}</span>}
      </div>
      {changes.length > 0 && (
        <ul className="mt-0.5 flex flex-col gap-0.5">
          {changes.map((change) => (
            <li key={change.field} className="break-words">
              <ChangeLine change={change} t={t} language={language} />
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

function Actor({
  entry,
  profiles,
  profilesSettled,
  t,
}: {
  entry: EventHistoryEntry;
  profiles: MemberProfile[] | undefined;
  profilesSettled: boolean;
  t: Translate;
}) {
  // 管理コンソールの操作者はサーバーのメンバーとは限らないので、名前を引かずに「運営」と出す
  if (entry.source === "admin") {
    return <span className="font-medium">{t("DisCalendar の運営")}</span>;
  }
  const id = entry.actor_discord_user_id;
  if (!id) return <span className="font-medium">{t("記録なし")}</span>;
  const profile = profiles?.find((member) => member.user_id === id);
  if (!profile) {
    return (
      <span className="font-medium">
        {profilesSettled ? t("メンバー情報を取得できません") : t("読み込み中…")}
      </span>
    );
  }
  return (
    <span className="flex min-w-0 items-center gap-1.5 font-medium">
      {profile.avatar_url && (
        // biome-ignore lint/performance/noImgElement: Discord の小さなアバターを直接表示する
        <img
          src={profile.avatar_url}
          alt=""
          className="size-5 shrink-0 rounded-full"
        />
      )}
      <span className="min-w-0 break-words">
        {profile.display_name ?? t("退出したメンバー")}
      </span>
    </span>
  );
}

function ChangeLine({
  change,
  t,
  language,
}: {
  change: EventHistoryChange;
  t: Translate;
  language: Language;
}) {
  const none = t("なし");
  switch (change.field) {
    case "name":
      return (
        <>
          {t("タイトル: 「{before}」→「{after}」", {
            before: change.before,
            after: change.after,
          })}
        </>
      );
    case "schedule":
      return (
        <>
          {t("日時: {before} → {after}", {
            before: describeEventRange(change.before, language),
            after: describeEventRange(change.after, language),
          })}
        </>
      );
    case "location":
      return (
        <>
          {t("場所: {before} → {after}", {
            before: change.before ?? none,
            after: change.after ?? none,
          })}
        </>
      );
    case "description":
      return <>{t("説明を変更")}</>;
    case "color":
      return (
        <span className="inline-flex items-center gap-1">
          {t("色:")}
          <ColorSwatch color={change.before} />→
          <ColorSwatch color={change.after} />
        </span>
      );
    case "notifications": {
      const describe = (list: typeof change.before) =>
        list.length
          ? list.map((n) => describeNotification(n, language)).join(t("・"))
          : none;
      return (
        <>
          {t("通知: {before} → {after}", {
            before: describe(change.before),
            after: describe(change.after),
          })}
        </>
      );
    }
    case "recurrence":
      return <>{t("繰り返しの設定を変更")}</>;
    case "mentions":
      return <>{t("通知のメンション先を変更")}</>;
    case "discord":
      return (
        <>
          {change.after
            ? t("Discord のイベントと連携")
            : t("Discord のイベントとの連携を解除")}
        </>
      );
  }
}

function ColorSwatch({ color }: { color: string }) {
  return (
    <span
      role="img"
      aria-label={color}
      className="inline-block size-3.5 rounded-sm border"
      style={{ backgroundColor: color }}
    />
  );
}
