"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { COLOR_SWATCHES } from "@/lib/event-form";
import { cn } from "@/lib/utils";

interface Props {
  id?: string;
  /** #RRGGBB */
  value: string;
  onChange: (color: string) => void;
  invalid?: boolean;
  className?: string;
}

/** 色の選択 (旧 v-color-picker 相当)。swatches から選ぶか、ネイティブのカラーピッカーで自由に指定する */
export function ColorPicker({
  id,
  value,
  onChange,
  invalid,
  className,
}: Props) {
  const [open, setOpen] = useState(false);
  const normalized = value.toUpperCase();
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        render={
          <Button
            id={id}
            variant="outline"
            aria-invalid={invalid || undefined}
            className={cn("w-full justify-start gap-2 font-normal", className)}
          />
        }
      >
        <span
          aria-hidden
          className="size-4 shrink-0 rounded-full border border-foreground/20"
          style={{ backgroundColor: value }}
        />
        <span className="font-mono text-xs">{normalized}</span>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-auto gap-3">
        <div className="grid grid-cols-4 gap-2">
          {COLOR_SWATCHES.map((color) => (
            <button
              key={color}
              type="button"
              aria-label={color}
              aria-pressed={color === normalized}
              onClick={() => {
                onChange(color);
                setOpen(false);
              }}
              className="size-7 rounded-full border border-foreground/20 transition-transform hover:scale-110 focus-visible:outline-2 focus-visible:outline-ring aria-pressed:ring-2 aria-pressed:ring-foreground aria-pressed:ring-offset-2 aria-pressed:ring-offset-popover"
              style={{ backgroundColor: color }}
            />
          ))}
        </div>
        <label className="flex items-center justify-between gap-3 text-xs text-muted-foreground">
          その他の色
          <input
            type="color"
            value={value}
            onChange={(e) => onChange(e.target.value.toUpperCase())}
            className="h-7 w-12 cursor-pointer rounded-md border border-input bg-transparent p-0.5"
          />
        </label>
      </PopoverContent>
    </Popover>
  );
}
