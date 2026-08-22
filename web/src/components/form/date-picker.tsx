"use client";

import { format } from "date-fns";
import { ja } from "date-fns/locale";
import { CalendarIcon } from "lucide-react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";

interface Props {
  id?: string;
  value: Date;
  onChange: (date: Date) => void;
  disabled?: boolean;
  invalid?: boolean;
  className?: string;
}

/** 日付入力 (旧 v-date-picker 相当)。ボタンを押すとカレンダーがポップオーバーで開く */
export function DatePicker({
  id,
  value,
  onChange,
  disabled,
  invalid,
  className,
}: Props) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            id={id}
            variant="outline"
            disabled={disabled}
            aria-invalid={invalid || undefined}
            className={cn("w-full justify-start font-normal", className)}
          />
        }
      >
        <CalendarIcon
          data-icon="inline-start"
          className="text-muted-foreground"
        />
        {format(value, "yyyy/MM/dd (E)", { locale: ja })}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-auto p-0">
        <Calendar
          mode="single"
          locale={ja}
          selected={value}
          defaultMonth={value}
          onSelect={(date) => {
            if (!date) return;
            onChange(date);
            setOpen(false);
          }}
        />
      </PopoverContent>
    </Popover>
  );
}
