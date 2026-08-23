import { ExternalLinkIcon } from "lucide-react";
import Link from "next/link";
import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

interface Props {
  href: string;
  children: ReactNode;
  /** primary: 目立たせる (Bot の招待など)、outline: 補助的な導線 */
  variant?: "primary" | "outline";
  /** 別タブで開く (Discord の画面など)。既定は http(s) で始まる URL のとき */
  external?: boolean;
  className?: string;
}

/** docs 本文中のボタン型リンク (旧 Btn 相当)。LP のボタンと同じ見た目にする */
export function LinkButton({
  href,
  children,
  variant = "outline",
  external = /^https?:/.test(href),
  className,
}: Props) {
  const classes = cn(
    "my-2 inline-flex h-11 items-center justify-center gap-2 rounded-full px-6 text-sm font-semibold tracking-wide no-underline transition-colors",
    variant === "primary"
      ? "bg-indigo-500 text-white hover:bg-indigo-400"
      : "border border-white/20 text-neutral-100 hover:bg-white/10",
    className,
  );
  if (external) {
    return (
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer"
        className={classes}
      >
        {children}
        <ExternalLinkIcon className="size-4" aria-hidden />
      </a>
    );
  }
  return (
    <Link href={href} className={classes}>
      {children}
    </Link>
  );
}
