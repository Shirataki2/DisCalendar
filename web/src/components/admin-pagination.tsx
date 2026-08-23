import Link from "next/link";

/**
 * 管理コンソールの一覧 (ギルド / ユーザー / 監査ログ) で使うページ送り。
 * 検索条件とページは URL に持たせて RSC で取得しているので、リンクだけで完結する
 * (Server Component のまま使える)
 */
export function AdminPagination({
  page,
  lastPage,
  href,
}: {
  /** 1 始まりの現在ページ */
  page: number;
  lastPage: number;
  /** ページ番号から遷移先の URL を作る */
  href: (page: number) => string;
}) {
  if (lastPage <= 1) return null;
  return (
    <nav
      aria-label="ページ"
      className="mt-4 flex items-center justify-between text-sm"
    >
      {page > 1 ? (
        <Link
          href={href(page - 1)}
          className="rounded-md border border-white/15 px-3 py-1 hover:bg-white/10"
        >
          前のページ
        </Link>
      ) : (
        <span />
      )}
      <span className="text-neutral-400">
        {page} / {lastPage} ページ
      </span>
      {page < lastPage ? (
        <Link
          href={href(page + 1)}
          className="rounded-md border border-white/15 px-3 py-1 hover:bg-white/10"
        >
          次のページ
        </Link>
      ) : (
        <span />
      )}
    </nav>
  );
}
