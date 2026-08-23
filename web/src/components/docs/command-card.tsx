import type { ReactNode } from "react";

interface Props {
  /** コマンド名 (例: "/init") */
  name: string;
  /** 引数の表記 (例: "[チャンネル]") */
  args?: string;
  /** 実行に必要な権限の補足 */
  permission?: string;
  children: ReactNode;
}

/** 「利用可能なコマンド」の 1 コマンド分のカード (旧 Command.vue 相当) */
export function CommandCard({ name, args, permission, children }: Props) {
  return (
    <section className="my-6 overflow-hidden rounded-xl border border-white/10 bg-surface">
      <h3 className="flex flex-wrap items-baseline gap-x-3 gap-y-1 border-b border-white/10 px-5 py-3">
        <code className="font-mono text-base font-semibold text-white">
          {name}
          {args && (
            <span className="ml-2 font-normal text-neutral-400">{args}</span>
          )}
        </code>
        {permission && (
          <span className="rounded-full bg-amber-500/15 px-2.5 py-0.5 text-xs font-medium text-amber-200">
            {permission}
          </span>
        )}
      </h3>
      <div className="px-5 py-1 text-sm [&_p]:my-3 [&_ul]:my-2">{children}</div>
    </section>
  );
}
