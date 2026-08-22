"use client";

import type { Popover as PopoverPrimitive } from "@base-ui/react/popover";
import { AlarmClockIcon, BellIcon, PencilIcon, Trash2Icon } from "lucide-react";
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
  onEdit: (event: ApiEvent) => void;
  onDelete: (event: ApiEvent) => void;
  onClose: () => void;
}

/** 予定をクリックしたときの概要ポップオーバー (旧 SimpleEdit.vue 相当) */
export function EventPopover({
  event,
  anchor,
  canEdit,
  onEdit,
  onDelete,
  onClose,
}: Props) {
  // 閉じるアニメーションの間も直前の内容を出しておく
  const shown = useLastValue(event);
  const shownAnchor = useLastValue(anchor);
  if (!shown) return null;

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
        className="w-80 gap-0 overflow-hidden p-0"
      >
        <div
          className="px-4 py-2.5 font-semibold"
          style={{
            backgroundColor: shown.color,
            color: readableTextColor(shown.color),
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
          {shown.description && (
            <p className="mt-1 max-h-40 overflow-y-auto whitespace-pre-wrap break-words text-xs text-muted-foreground">
              {shown.description}
            </p>
          )}
        </div>
        {canEdit && (
          <div className="flex items-center justify-between border-t px-2 py-1.5">
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
