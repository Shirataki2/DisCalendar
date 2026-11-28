"use client";

import { InfoIcon } from "lucide-react";
import { useId, useState } from "react";
import { useLanguage } from "@/components/language-provider";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

/** ホバー・キーボードのフォーカスに加え、タッチではタップで補足を開く。 */
export function SettingsHint({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  const { t } = useLanguage();
  const [open, setOpen] = useState(false);
  const triggerId = useId();
  return (
    <TooltipProvider>
      <Tooltip open={open} onOpenChange={setOpen} triggerId={triggerId}>
        <TooltipTrigger
          id={triggerId}
          type="button"
          aria-label={t("{label}の補足", { label })}
          aria-describedby={open ? `${triggerId}-content` : undefined}
          closeOnClick={false}
          onClick={() => setOpen(true)}
          className="inline-flex size-6 shrink-0 items-center justify-center rounded text-muted-foreground focus-visible:outline-2 focus-visible:outline-ring"
        >
          <InfoIcon aria-hidden className="size-4" />
        </TooltipTrigger>
        <TooltipContent
          id={`${triggerId}-content`}
          role="tooltip"
          className="max-w-[min(20rem,calc(100vw-2rem))] leading-relaxed"
        >
          {children}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}
