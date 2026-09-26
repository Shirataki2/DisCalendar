import { rules, SimpleMarkdown } from "discord-markdown-parser";
import { createElement, Fragment, type ReactNode } from "react";
import { locationUrl } from "@/lib/event-location";
import { cn } from "@/lib/utils";

// HTML・画像・メンションの解決は許可せず、必要な構文だけを解析する。
const parse = SimpleMarkdown.parserFor({
  blockQuote: rules.blockQuote,
  codeBlock: rules.codeBlock,
  escape: rules.escape,
  autolink: rules.autolink,
  url: rules.url,
  link: SimpleMarkdown.defaultRules.link,
  em: rules.em,
  strong: rules.strong,
  underline: rules.underline,
  strikethrough: rules.strikethrough,
  inlineCode: rules.inlineCode,
  text: rules.text,
  br: rules.br,
  spoiler: rules.spoiler,
  heading: rules.heading,
  paragraph: SimpleMarkdown.defaultRules.paragraph,
  list: {
    ...SimpleMarkdown.defaultRules.list,
    // Discord のように空行なしでも行頭からリストを始められるようにする。
    match: (source, state, previous) => {
      const capture = SimpleMarkdown.defaultRules.list.match(
        source,
        { ...state, inline: false },
        previous,
      );
      if (!capture) return null;
      // 標準ルールは種類の違うリストや次の通常行まで取り込むため、境界で止める。
      for (const line of capture[0].matchAll(/\n( *)(\S[^\n]*)/g)) {
        if (line[1].length > capture[1].length) continue;
        const bullet = /^(?:[*+-]|\d+\.) /.exec(line[2]);
        if (!bullet || /^\d/.test(bullet[0]) !== /^\d/.test(capture[2])) {
          capture[0] = capture[0].slice(0, line.index + 1);
          break;
        }
      }
      return capture;
    },
  },
});

type Node = ReturnType<typeof parse>[number];

function renderNodes(nodes: Node[]): ReactNode {
  // 入力順だけで決まる静的な構文木なので、兄弟間の位置をキーにする。
  return nodes.map((node, index) => (
    // biome-ignore lint/suspicious/noArrayIndexKey: 説明全体を文字列の変更時に再マウントする静的な構文木
    <Fragment key={index}>{renderNode(node)}</Fragment>
  ));
}

function containsInteractive(nodes: Node[]): boolean {
  return nodes.some(
    (node) =>
      ["link", "url", "autolink", "spoiler"].includes(node.type) ||
      (Array.isArray(node.content) && containsInteractive(node.content)),
  );
}

function renderNode(node: Node): ReactNode {
  switch (node.type) {
    case "text":
      return node.content;
    case "br":
    case "newline":
      return <br />;
    case "strong":
      return <strong>{renderNodes(node.content)}</strong>;
    case "em":
      return <em>{renderNodes(node.content)}</em>;
    case "underline":
      return <u>{renderNodes(node.content)}</u>;
    case "strikethrough":
      return <del>{renderNodes(node.content)}</del>;
    case "inlineCode":
      return <code className="rounded bg-muted px-1">{node.content}</code>;
    case "codeBlock":
      return (
        <pre className="my-2 max-w-full overflow-x-auto rounded bg-muted p-2 whitespace-pre">
          <code>{node.content}</code>
        </pre>
      );
    case "heading":
      return createElement(
        node.level === 1 ? "h3" : node.level === 2 ? "h4" : "h5",
        {
          className: cn(
            "my-2 font-semibold",
            node.level === 1
              ? "text-[1.25em]"
              : node.level === 2
                ? "text-[1.125em]"
                : "text-[1em]",
          ),
        },
        renderNodes(node.content),
      );
    case "paragraph":
      return <div className="my-2">{renderNodes(node.content)}</div>;
    case "blockQuote":
      return (
        <blockquote className="my-2 border-l-2 border-muted-foreground/40 pl-3">
          {renderNodes(node.content)}
        </blockquote>
      );
    case "list": {
      const Tag = node.ordered ? "ol" : "ul";
      return (
        <Tag
          start={node.ordered ? node.start : undefined}
          className={cn(
            "my-2 pl-5",
            node.ordered ? "list-decimal" : "list-disc",
          )}
        >
          {node.items.map((item: Node[], index: number) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: 説明全体を文字列の変更時に再マウントする静的な構文木
            <li key={index}>{renderNodes(item)}</li>
          ))}
        </Tag>
      );
    }
    case "spoiler":
      return (
        <details className="inline-block max-w-full rounded bg-muted align-top">
          <summary className="min-h-11 cursor-pointer content-center rounded px-2 text-sm focus-visible:outline-2 focus-visible:outline-ring">
            スポイラー
          </summary>
          <div className="px-2 pb-2">{renderNodes(node.content)}</div>
        </details>
      );
    case "link":
    case "url":
    case "autolink": {
      const href = locationUrl(node.target);
      const content = renderNodes(node.content);
      // リンクや折りたたみをリンクで囲むと HTML が壊れるため、外側は文字だけにする。
      return href && !containsInteractive(node.content) ? (
        <a
          href={href}
          target="_blank"
          rel="noopener noreferrer nofollow"
          className="text-primary underline underline-offset-4 focus-visible:outline-2 focus-visible:outline-ring"
        >
          {content}
        </a>
      ) : (
        content
      );
    }
    default:
      return null;
  }
}

/** 保存済みの文字列を、安全な React 要素だけで表示する。 */
export function EventDescription({
  children,
  className,
}: {
  children: string;
  className?: string;
}) {
  return (
    <div
      key={children}
      data-slot="event-description"
      className={cn(
        "min-w-0 whitespace-pre-wrap [overflow-wrap:anywhere] leading-relaxed",
        className,
      )}
    >
      {renderNodes(parse(children, { inline: true }))}
    </div>
  );
}
