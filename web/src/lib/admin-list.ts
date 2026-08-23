// 管理コンソールの一覧ページ (ギルド / ユーザー / 監査ログ) 共通の URL 引数の扱い。
// 検索条件とページ番号は URL (searchParams) に持たせて RSC で取得している
// (件数が少なく、共有・再読込しやすい方が運用には向くため)。

/** `searchParams` の値 (string | string[] | undefined) から 1 つ取り出す */
export function firstParam(value: string | string[] | undefined): string {
  return (Array.isArray(value) ? value[0] : value) ?? "";
}

/** URL の `page` を api が受け付ける範囲 (`1..=maxPage`) に正規化する。不正な値は 1 ページ目 */
export function parsePage(value: string, maxPage: number): number {
  const n = Number.parseInt(value, 10);
  if (!Number.isFinite(n) || n < 1) return 1;
  return Math.min(n, maxPage);
}

/** 総件数と 1 ページの件数から最後のページ番号を出す (0 件でも 1 ページ目) */
export function lastPageOf(total: number, pageSize: number): number {
  return Math.max(1, Math.ceil(total / pageSize));
}

/** 一覧の URL を作る (既定値 = 空文字 / 1 ページ目 は付けない) */
export function listHref(
  path: string,
  params: Record<string, string>,
  page: number,
): string {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value) query.set(key, value);
  }
  if (page > 1) query.set("page", String(page));
  const search = query.toString();
  return search ? `${path}?${search}` : path;
}
