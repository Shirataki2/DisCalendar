"use client";

import type { Popover as PopoverPrimitive } from "@base-ui/react/popover";
import {
  AlarmClockIcon,
  ArrowRightIcon,
  BellIcon,
  CopyIcon,
  HistoryIcon,
  MapPinIcon,
  PencilIcon,
  Trash2Icon,
} from "lucide-react";
import Link from "next/link";
import { type ReactNode, useState } from "react";
import { EventAttachments } from "@/components/event-attachments";
import { EventAuthors } from "@/components/event-authors";
import { EventDescription } from "@/components/event-description";
import { EventHistoryDialog } from "@/components/event-history-dialog";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent } from "@/components/ui/popover";
import { useLastValue } from "@/hooks/use-last-value";
import type { ApiEvent } from "@/lib/api/types";
import {
  describeEventRange,
  describeNotification,
} from "@/lib/calendar-events";
import { readableTextColor } from "@/lib/color";
import { locationUrl } from "@/lib/event-location";
import { describeRecurrence } from "@/lib/recurrence";

/** ポップオーバーを寄せる先。要素そのもののほか、位置だけを持つ仮想要素も渡せる */
export type PopoverAnchor = NonNullable<
  PopoverPrimitive.Positioner.Props["anchor"]
>;

interface Props {
  guidance?: ReactNode;
  /** null なら閉じている */
  event: ApiEvent | null;
  anchor: PopoverAnchor | null;
  canEdit: boolean;
  resolveAuthors?: boolean;
  allowAttachments?: boolean;
  /** 変更履歴 (#165) を開けるか。通常の API で読めない予定 (管理コンソール・チュートリアル) では false */
  allowHistory?: boolean;
  /** ヘッダの色。横断カレンダー (#98) ではサーバーの色で塗る (省略時は予定の色) */
  color?: string;
  /** 横断カレンダー (#98) で出すサーバーの行と、そのサーバーのカレンダーへのリンク */
  guild?: {
    name: string;
    iconUrl: string | null;
    href: string;
  };
  /** 編集の操作。canEdit のときだけ使う (閲覧専用の呼び出し側は渡さなくてよい) */
  onEdit?: (event: ApiEvent) => void;
  onDuplicate?: (event: ApiEvent) => void;
  onDelete?: (event: ApiEvent) => void;
  onClose: () => void;
}

/** 予定をクリックしたときの概要ポップオーバー (旧 SimpleEdit.vue 相当) */
export function EventPopover({
  guidance,
  event,
  anchor,
  canEdit,
  resolveAuthors = true,
  allowAttachments = true,
  allowHistory = true,
  color,
  guild,
  onEdit,
  onDuplicate,
  onDelete,
  onClose,
}: Props) {
  const { t, language } = useLanguage();
  // 閉じるアニメーションの間も直前の内容を出しておく
  const shown = useLastValue(event);
  const shownAnchor = useLastValue(anchor);
  const shownColor = useLastValue(color ?? null);
  const shownGuild = useLastValue(guild ?? null);
  // 履歴ダイアログはポップオーバーを閉じてから開くので、ポップオーバーの外に置いて対象を別に持つ
  const [historyTarget, setHistoryTarget] = useState<{
    guildId: string;
    eventId: number;
  } | null>(null);
  const shownHistoryTarget = useLastValue(historyTarget);
  if (!shown) return null;
  const showActions = canEdit && !!onEdit && !!onDuplicate && !!onDelete;
  const openHistory = () => {
    setHistoryTarget({ guildId: shown.guild_id, eventId: shown.id });
    onClose();
  };
  const headerColor = shownColor ?? shown.color;

  const notifications = shown.notifications.length
    ? shown.notifications
        .map((notification) => describeNotification(notification, language))
        .join(t("・"))
    : "-";

  return (
    <>
      <Popover
        open={event !== null}
        onOpenChange={(open) => {
          if (!open) onClose();
        }}
      >
        <PopoverContent
          anchor={shownAnchor}
          side="right"
          align="start"
          sideOffset={8}
          // 幅 320px 未満の端末でも画面からはみ出さないようにする (#14)
          className="w-80 max-w-[calc(100vw-1rem)] max-h-[calc(100dvh-2rem)] gap-0 overflow-y-auto p-0"
        >
          <div
            className="px-4 py-2.5 font-semibold"
            style={{
              backgroundColor: headerColor,
              color: readableTextColor(headerColor),
            }}
          >
            {shown.name}
          </div>
          <div className="flex flex-col gap-1.5 px-4 py-3">
            {shown.recurrence && (
              <p className="text-sm">
                {describeRecurrence(shown.recurrence.rule, language)}
                {shown.recurrence.is_exception ? t("（この回は個別変更）") : ""}
              </p>
            )}
            <div className="flex items-center gap-2">
              <AlarmClockIcon className="size-4 shrink-0 text-muted-foreground" />
              <span>{describeEventRange(shown, language)}</span>
            </div>
            {shown.location && (
              <div className="flex items-start gap-2">
                <MapPinIcon className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                {locationUrl(shown.location) ? (
                  <a
                    href={shown.location}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="min-w-0 break-all text-primary underline underline-offset-4"
                  >
                    {shown.location}
                  </a>
                ) : (
                  <span className="min-w-0 break-words">{shown.location}</span>
                )}
              </div>
            )}
            <div className="flex items-center gap-2">
              <BellIcon className="size-4 shrink-0 text-muted-foreground" />
              <span>{notifications}</span>
            </div>
            {shownGuild && (
              <div className="flex items-center gap-2">
                {shownGuild.iconUrl ? (
                  // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
                  <img
                    src={shownGuild.iconUrl}
                    alt=""
                    className="size-4 shrink-0 rounded-full"
                  />
                ) : (
                  <span className="flex size-4 shrink-0 items-center justify-center rounded-full bg-foreground/10 text-[0.6rem] font-bold">
                    {shownGuild.name.slice(0, 1)}
                  </span>
                )}
                <span className="truncate">{shownGuild.name}</span>
              </div>
            )}
            <EventAuthors
              event={shown}
              active={event !== null}
              resolveMembers={resolveAuthors}
            />
            {/* 編集できる人には下の操作行にアイコンで出し、狭い画面で概要の高さを増やさない */}
            {allowHistory && !showActions && (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="-ml-2 self-start text-muted-foreground"
                onClick={openHistory}
              >
                <HistoryIcon />
                {t("変更履歴")}
              </Button>
            )}
            {allowAttachments && (
              <EventAttachments
                key={shown.id}
                guildId={shown.guild_id}
                eventId={shown.id}
                active={event !== null}
              />
            )}
            {shown.description && (
              <EventDescription className="mt-1 max-h-40 overflow-y-auto text-xs text-muted-foreground">
                {shown.description}
              </EventDescription>
            )}
          </div>
          {guidance && <div className="px-3 pb-3">{guidance}</div>}
          {shownGuild && (
            <div className="border-t px-2 py-1.5">
              <Link
                href={shownGuild.href}
                className="flex items-center gap-1.5 rounded-md px-2 py-1 text-sm hover:bg-foreground/10"
              >
                {t("このサーバーのカレンダーを開く")}
                <ArrowRightIcon className="size-4" aria-hidden />
              </Link>
            </div>
          )}
          {showActions && onEdit && onDuplicate && onDelete && (
            <div className="flex items-center justify-between border-t px-2 py-1.5">
              <div className="flex items-center">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => onEdit(shown)}
                >
                  <PencilIcon />

                  {t("編集")}
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => onDuplicate(shown)}
                >
                  <CopyIcon />

                  {t("複製")}
                </Button>
              </div>
              <div className="flex items-center">
                {allowHistory && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-sm"
                    aria-label={t("変更履歴")}
                    title={t("変更履歴")}
                    onClick={openHistory}
                  >
                    <HistoryIcon />
                  </Button>
                )}
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="text-destructive hover:text-destructive"
                  onClick={() => onDelete(shown)}
                >
                  <Trash2Icon />

                  {t("削除")}
                </Button>
              </div>
            </div>
          )}
        </PopoverContent>
      </Popover>
      {shownHistoryTarget && (
        <EventHistoryDialog
          guildId={shownHistoryTarget.guildId}
          eventId={shownHistoryTarget.eventId}
          open={historyTarget !== null}
          onOpenChange={(open) => {
            if (!open) setHistoryTarget(null);
          }}
        />
      )}
    </>
  );
}
