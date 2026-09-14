import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import { McpConsentForm } from "./mcp-consent-form";

test("同意の初期表示は要求された既知scopeだけを全選択し、空サーバーを案内する", () => {
  const html = renderToStaticMarkup(
    createElement(McpConsentForm, {
      query: "scope=events%3Aread+offline_access+unknown",
      guilds: [],
    }),
  );
  expect(html.match(/checked=""/g)).toHaveLength(2);
  expect(html).toContain("許可できるサーバーがありません");
  expect(html).toContain("許可する操作とサーバーをそれぞれ1件以上");
  expect(html).not.toContain('value="unknown"');
  expect(html).not.toContain('value="events:delete"');
});

test("サーバーが100件を超えると全選択を維持しつつ絞り込みを案内する", () => {
  const html = renderToStaticMarkup(
    createElement(McpConsentForm, {
      query: "scope=guilds%3Aread",
      guilds: Array.from({ length: 101 }, (_, i) => ({
        id: String(i + 1),
        name: `サーバー${i + 1}`,
      })),
    }),
  );
  expect(html.match(/checked=""/g)).toHaveLength(102);
  expect(html).toContain("サーバーは100件まで許可できます");
});
