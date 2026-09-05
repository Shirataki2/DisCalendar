"use client";

import type { Popover as PopoverPrimitive } from "@base-ui/react/popover";
import {
  AlarmClockIcon,
  ArrowRightIcon,
  BellIcon,
  CopyIcon,
  PencilIcon,
  Trash2Icon,
} from "lucide-react";
import Link from "next/link";
import { EventAuthors } from "@/components/event-authors";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent } from "@/components/ui/popover";
import { useLastValue } from "@/hooks/use-last-value";
import type { ApiEvent } from "@/lib/api/types";
import {
  describeEventRange,
  describeNotification,
} from "@/lib/calendar-events";
import { readableTextColor } from "@/lib/color";

/** ポップオーバーを寄せる先。要素そのもののほか、位置だけを持つ仮想要素も渡せる */
export type PopoverAnchor = NonNullable<
  PopoverPrimitive.Positioner.Props["anchor"]
>;

interface Props {
  /** null なら閉じている */
  event: ApiEvent | null;
  anchor: PopoverAnchor | null;
  canEdit: boolean;
  resolveAuthors?: boolean;
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
  event,
  anchor,
  canEdit,
  resolveAuthors = true,
  color,
  guild,
  onEdit,
  onDuplicate,
  onDelete,
  onClose,
}: Props) {
  // 閉じるアニメーションの間も直前の内容を出しておく
  const shown = useLastValue(event);
  const shownAnchor = useLastValue(anchor);
  const shownColor = useLastValue(color ?? null);
  const shownGuild = useLastValue(guild ?? null);
  if (!shown) return null;
  const headerColor = shownColor ?? shown.color;

  const notifications = shown.notifications.length
    ? shown.notifications.map(describeNotification).join("・")
    : "-";

  return (
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
        className="w-80 max-w-[calc(100vw-1rem)] gap-0 overflow-hidden p-0"
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
          <div className="flex items-center gap-2">
            <AlarmClockIcon className="size-4 shrink-0 text-muted-foreground" />
            <span>{describeEventRange(shown)}</span>
          </div>
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
          {shown.description && (
            <p className="mt-1 max-h-40 overflow-y-auto whitespace-pre-wrap break-words text-xs text-muted-foreground">
              {shown.description}
            </p>
          )}
        </div>
        {shownGuild && (
          <div className="border-t px-2 py-1.5">
            <Link
              href={shownGuild.href}
              className="flex items-center gap-1.5 rounded-md px-2 py-1 text-sm hover:bg-foreground/10"
            >
              このサーバーのカレンダーを開く
              <ArrowRightIcon className="size-4" aria-hidden />
            </Link>
          </div>
        )}
        {canEdit && onEdit && onDuplicate && onDelete && (
          <div className="flex items-center justify-between border-t px-2 py-1.5">
            <div className="flex items-center">
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => onEdit(shown)}
              >
                <PencilIcon />
                編集
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => onDuplicate(shown)}
              >
                <CopyIcon />
                複製
              </Button>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="text-destructive hover:text-destructive"
              onClick={() => onDelete(shown)}
            >
              <Trash2Icon />
              削除
            </Button>
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
