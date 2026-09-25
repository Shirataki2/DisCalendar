"use client";

import type { CalendarRef } from "@fullcalendar/react";
import { CalendarSearchIcon } from "lucide-react";
import { type RefObject, useState } from "react";
// date-fns の ja に、年月のプルダウンなどの読み上げ用ラベル (「年を選択」など) を足したもの
import { ja } from "react-day-picker/locale";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";

/** 年のプルダウンに並べる範囲 (今日と表示中の日付の前後この年数) */
const YEAR_RANGE = 10;

interface Props {
  calendarRef: RefObject<CalendarRef | null>;
}

/**
 * ツールバーのタイトル横に置く「日付を指定して移動」ボタン (#59。旧版はタイトルのクリックで開いた)。
 * 日付を選ぶと、いまのビューのままその日付を含む期間へ移動する。
 * ピッカーは予定フォームの DatePicker (form/date-picker.tsx) と同じ Popover + Calendar で、
 * 遠い月へ一度に飛べるよう年月をプルダウンで選べるようにしている。
 * 開くたびに FullCalendar の表示中の日付を読み直すので、ほかの操作で移動した後も表示位置から始まる
 */
export function CalendarDateJump({ calendarRef }: Props) {
  const [open, setOpen] = useState(false);
  const [current, setCurrent] = useState<Date>(() => new Date());

  const today = new Date();
  const startMonth = new Date(
    Math.min(today.getFullYear(), current.getFullYear()) - YEAR_RANGE,
    0,
  );
  const endMonth = new Date(
    Math.max(today.getFullYear(), current.getFullYear()) + YEAR_RANGE,
    11,
  );

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        if (next) {
          setCurrent(calendarRef.current?.getApi().getDate() ?? new Date());
        }
        setOpen(next);
      }}
    >
      <PopoverTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label="日付を指定して移動"
            title="日付を指定して移動"
          />
        }
      >
        <CalendarSearchIcon />
      </PopoverTrigger>
      <PopoverContent align="center" className="w-auto p-0">
        <Calendar
          mode="single"
          required
          locale={ja}
          captionLayout="dropdown"
          startMonth={startMonth}
          endMonth={endMonth}
          selected={current}
          defaultMonth={current}
          onSelect={(date) => {
            if (!date) return;
            calendarRef.current?.getApi().gotoDate(date);
            setOpen(false);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}
