import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { DOC_PAGES } from "@/lib/docs";
import { english } from "@/lib/i18n/messages";
import {
  USECASE_COMMON_SOLUTIONS,
  USECASE_HERO_IMAGE,
  USECASE_PAGES,
} from "@/lib/usecases";

describe("用途別の紹介ページ", () => {
  test("導入元は api が受け付ける値で、クリック数が other にまとめられない", () => {
    const source = readFileSync(
      new URL("../../../api/src/models/invite_clicks.rs", import.meta.url),
      "utf8",
    );
    const known = source.match(/KNOWN_SOURCES[^=]*=\s*&\[([\s\S]*?)\];/)?.[1];
    expect(known).toBeDefined();
    for (const page of USECASE_PAGES) {
      expect(page.inviteSource).toBe(`usecase-${page.slug}`);
      expect(known).toContain(`"${page.inviteSource}"`);
    }
  });

  test("slug が重ならず、案内先の使い方ページが存在する", () => {
    const slugs = USECASE_PAGES.map((page) => page.slug);
    expect(new Set(slugs).size).toBe(slugs.length);
    const docSlugs = new Set(DOC_PAGES.map((page) => page.slug));
    for (const solution of [
      ...USECASE_PAGES.flatMap((page) => page.solutions),
      ...USECASE_COMMON_SOLUTIONS,
    ]) {
      expect(docSlugs, solution.title).toContain(solution.docSlug);
    }
  });

  test("画面に出す文章はすべて英訳がある", () => {
    const messages = [
      USECASE_HERO_IMAGE.alt,
      ...USECASE_COMMON_SOLUTIONS.flatMap((s) => [s.title, s.body]),
      ...USECASE_PAGES.flatMap((page) => [
        page.name,
        page.headline,
        page.lead,
        page.image.alt,
        ...page.problems,
        ...page.solutions.flatMap((s) => [s.title, s.body]),
      ]),
    ];
    for (const message of messages) {
      expect(Object.hasOwn(english, message), message).toBe(true);
    }
  });
});
