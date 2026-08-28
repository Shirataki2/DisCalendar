import type { MDXComponents } from "mdx/types";
import Link from "next/link";
import type { ComponentPropsWithoutRef } from "react";

// @next/mdx が MDX の描画に使うコンポーネントの対応表 (App Router では必須のファイル)。
// docs (src/content/docs/*.mdx) の見出し・段落・リンクなどをサイトの配色に合わせる。
// Tailwind の typography プラグインは入れず、使っている要素だけをここで指定する

function Anchor({ href, children, ...rest }: ComponentPropsWithoutRef<"a">) {
  const className =
    "text-indigo-300 underline underline-offset-4 transition-colors hover:text-indigo-200";
  if (href?.startsWith("/")) {
    return (
      <Link href={href} className={className} {...rest}>
        {children}
      </Link>
    );
  }
  return (
    <a
      href={href}
      className={className}
      target="_blank"
      rel="noopener noreferrer"
      {...rest}
    >
      {children}
    </a>
  );
}

const components = {
  h2: (props) => (
    <h2
      className="mt-12 mb-4 scroll-mt-20 border-b border-white/10 pb-2 text-xl font-bold first:mt-0 sm:text-2xl"
      {...props}
    />
  ),
  h3: (props) => (
    <h3 className="mt-8 mb-3 scroll-mt-20 text-lg font-semibold" {...props} />
  ),
  h4: (props) => (
    <h4
      className="mt-6 mb-2 scroll-mt-20 text-base font-semibold text-neutral-300"
      {...props}
    />
  ),
  p: (props) => (
    <p className="my-4 leading-8 text-neutral-200 sm:text-[15px]" {...props} />
  ),
  a: Anchor,
  ul: (props) => (
    <ul
      className="my-4 list-disc space-y-1.5 pl-6 leading-7 text-neutral-200"
      {...props}
    />
  ),
  ol: (props) => (
    <ol
      className="my-4 list-decimal space-y-1.5 pl-6 leading-7 text-neutral-200"
      {...props}
    />
  ),
  li: (props) => <li className="pl-1" {...props} />,
  strong: (props) => <strong className="font-semibold text-white" {...props} />,
  code: (props) => (
    <code
      className="rounded bg-white/10 px-1.5 py-0.5 font-mono text-[0.9em] text-neutral-100"
      {...props}
    />
  ),
  pre: (props) => (
    <pre
      className="my-4 overflow-x-auto rounded-lg border border-white/10 bg-surface px-4 py-3 font-mono text-sm leading-6 [&>code]:bg-transparent [&>code]:p-0 [&>code]:text-[1em]"
      {...props}
    />
  ),
  blockquote: (props) => (
    <blockquote
      className="my-4 border-l-4 border-indigo-400/60 bg-indigo-500/10 px-4 py-2 text-neutral-200 [&>p]:my-1"
      {...props}
    />
  ),
  table: (props) => (
    <div className="my-4 overflow-x-auto">
      <table className="w-full border-collapse text-sm" {...props} />
    </div>
  ),
  th: (props) => (
    <th
      className="border-b border-white/15 px-3 py-2 text-left font-semibold"
      {...props}
    />
  ),
  td: (props) => (
    <td
      className="border-b border-white/10 px-3 py-2 align-top leading-7"
      {...props}
    />
  ),
  hr: () => <hr className="my-8 border-white/10" />,
} satisfies MDXComponents;

export function useMDXComponents(): MDXComponents {
  return components;
}
