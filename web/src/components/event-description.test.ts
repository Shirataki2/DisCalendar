import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { EventDescription } from "./event-description";

const render = (children: string) =>
  renderToStaticMarkup(createElement(EventDescription, { children }));

describe("予定の説明", () => {
  it("Discord の書式と入れ子を描画する", () => {
    const html = render(
      "# 見出し\n## 小見出し\n### 詳細\n__**太字**__ *斜体* _斜体2_ ~~削除~~\n- 項目\n  - 子項目\n\n3. 番号\n4. 次\n\n> 引用\n\n`__コード__`\n```js\n**コード**\n```\n||秘密 **重要**||\n[案内](https://example.com/guide)",
    );
    for (const fragment of [
      ">見出し</h3>",
      ">小見出し</h4>",
      ">詳細</h5>",
      "<u><strong>太字</strong></u>",
      "<em>斜体</em>",
      "<em>斜体2</em>",
      "<del>削除</del>",
      "<ul",
      "項目",
      "<li>子項目</li>",
      '<ol start="3"',
      "<li>番号</li>",
      "<blockquote",
      "引用",
      "__コード__</code>",
      "<code>**コード**</code>",
      "<details",
      "秘密 <strong>重要</strong>",
      'href="https://example.com/guide"',
      'target="_blank"',
      'rel="noopener noreferrer nofollow"',
    ])
      expect(html).toContain(fragment);
    expect(html).not.toContain("<details open");
  });

  it("プレーンテキストの改行・空行・空白と未対応のメンションを保つ", () => {
    const html = render(
      "一行目\n二行目\n\n  空白\n<@123456789012345678> <:face:123456789012345678>",
    );
    expect(html).toContain("一行目<br/>二行目<br/><br/>  空白<br/>");
    expect(html).toContain("&lt;@123456789012345678&gt;");
    expect(html).toContain("&lt;:face:123456789012345678&gt;");
    expect(render("\\*太字ではない\\*")).toContain("*太字ではない*");
  });

  it("HTML・画像・スクリプトを要素にせず、危険な URL をリンクにしない", () => {
    for (const input of [
      '<script>alert(1)</script><img src=x onerror="alert(1)">',
      '<svg onload="alert(1)"></svg><iframe srcdoc="danger"></iframe>',
      "[危険](javascript:alert%281%29)",
      "[危険](JaVaScRiPt:alert%281%29)",
      "[危険](data:text/html,danger)",
      "[危険](vbscript:danger)",
      "[危険](java\nscript:danger)",
      "[危険](javascript&#58;danger)",
      "[危険](//example.com)",
      "![画像](https://example.com/track.png)",
    ]) {
      const html = render(input);
      expect(html).not.toMatch(/<(?:script|img|svg|iframe)\b/);
      expect(html).not.toMatch(/href="(?:javascript|data|vbscript|\/\/)/i);
    }
    expect(render("[危険](javascript:alert%281%29)")).not.toContain("<a ");
    expect(render("<script>alert(1)</script>")).toContain("&lt;script&gt;");
  });

  it("リンクやスポイラーをリンクで囲まず、共有ページの HTML を保つ", () => {
    const html = render(
      "[外側 [内側](https://example.com/inner)](https://example.com/outer)",
    );
    expect(html.match(/<a /g)).toHaveLength(1);
    expect(html).not.toContain('href="https://example.com/outer"');
    const spoiler = render("[||秘密||](https://example.com)");
    expect(spoiler).toContain("<details");
    expect(spoiler).not.toContain("<a ");
  });

  it("リスト後の通常行をリスト項目に含めない", () => {
    expect(render("- 項目\n通常行")).toContain("</ul>通常行");
    expect(render("1. 番号\n- 箇条書き")).toContain("</ol><ul");
  });

  it("URL を検証済みの正規化した値で描画し、コード内の書式は解釈しない", () => {
    expect(render("[案内](https://EXAMPLE.com/a)")).toContain(
      'href="https://example.com/a"',
    );
    expect(render("`||秘密|| __下線__`")).not.toContain("<details");
    expect(render("```\n<script>alert(1)</script>\n```")).toContain(
      "&lt;script&gt;",
    );
  });
});
