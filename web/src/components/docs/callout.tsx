import { InfoIcon, TriangleAlertIcon } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  /** info: 補足、warning: 注意 (権限が無いと操作できない、など) */
  type?: "info" | "warning";
  title?: string;
  children: ReactNode;
}

/** docs 本文中の補足・注意書き */
export function Callout({ type = "info", title, children }: Props) {
  const warning = type === "warning";
  return (
    <aside
      className={`my-6 flex gap-3 rounded-lg border px-4 py-3 text-sm leading-7 ${
        warning
          ? "border-amber-400/30 bg-amber-500/10 text-amber-50"
          : "border-indigo-400/30 bg-indigo-500/10 text-neutral-100"
      } [&_p]:my-0 [&_ul]:my-1`}
    >
      {warning ? (
        <TriangleAlertIcon
          className="mt-1 size-5 shrink-0 text-amber-300"
          aria-hidden
        />
      ) : (
        <InfoIcon
          className="mt-1 size-5 shrink-0 text-indigo-300"
          aria-hidden
        />
      )}
      <div className="min-w-0 flex-1">
        {title && <p className="mb-1 font-semibold">{title}</p>}
        {children}
      </div>
    </aside>
  );
}
