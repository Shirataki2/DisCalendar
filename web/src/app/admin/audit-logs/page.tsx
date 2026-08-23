import { SearchIcon } from "lucide-react";
import type { Metadata } from "next";
import { AdminPagination } from "@/components/admin-pagination";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { formatDateTime } from "@/lib/admin-format";
import { firstParam, lastPageOf, listHref, parsePage } from "@/lib/admin-list";
import { serverApi } from "@/lib/api/server";
import { ADMIN_AUDIT_LOGS_MAX_PAGE, type AdminAuditLog } from "@/lib/api/types";
import { ROUTES } from "@/lib/site";

export const metadata: Metadata = {
  title: "監査ログ | 管理コンソール",
};

/**
 * 監査ログの閲覧 (#37)。`/admin/*` の書き込み操作と SQL 実行が残した `admin_audit_logs` を新しい順に出す。
 * 絞り込み (action / actor) とページングは URL に持たせて RSC で取得する
 */
export default async function AdminAuditLogsPage({
  searchParams,
}: PageProps<"/admin/audit-logs">) {
  const params = await searchParams;
  const action = firstParam(params.action).trim();
  const actor = firstParam(params.actor).trim();
  const page = parsePage(firstParam(params.page), ADMIN_AUDIT_LOGS_MAX_PAGE);
  const result = await serverApi.admin.auditLogs.list(action, actor, page);
  const lastPage = lastPageOf(result.total, result.page_size);

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-xl font-bold">監査ログ</h1>
          <p className="mt-1 text-sm text-neutral-400">
            管理コンソールからの書き込み操作と SQL 実行の記録。{result.total} 件
          </p>
        </div>
        <form
          action={ROUTES.adminAuditLogs}
          method="get"
          className="flex flex-wrap items-center gap-2"
        >
          <label htmlFor="audit-action" className="sr-only">
            操作の種類
          </label>
          <select
            id="audit-action"
            name="action"
            defaultValue={action}
            className="h-9 rounded-md border border-white/15 bg-neutral-900 px-2 text-sm"
          >
            <option value="">すべての操作</option>
            {/* 選択肢は実際に記録されている action (api が DISTINCT で返す) */}
            {result.actions.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
          <Input
            type="search"
            name="actor"
            defaultValue={actor}
            placeholder="実行者の Discord ID"
            aria-label="実行者の Discord ユーザー ID"
            className="w-56"
          />
          <Button type="submit" variant="outline" size="sm">
            <SearchIcon data-icon="inline-start" />
            絞り込む
          </Button>
        </form>
      </div>

      {result.items.length === 0 ? (
        <p className="rounded-lg border border-white/10 px-3 py-8 text-center text-sm text-neutral-500">
          該当する記録はありません
        </p>
      ) : (
        <ul className="flex flex-col gap-2">
          {result.items.map((log) => (
            <AuditLogRow key={log.id} log={log} />
          ))}
        </ul>
      )}

      <AdminPagination
        page={page}
        lastPage={lastPage}
        href={(next) =>
          listHref(ROUTES.adminAuditLogs, { action, actor }, next)
        }
      />
    </main>
  );
}

function AuditLogRow({ log }: { log: AdminAuditLog }) {
  const hasSnapshot =
    log.before !== null || log.after !== null || log.detail !== null;
  return (
    <li className="rounded-lg border border-white/10 bg-white/5 p-3">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
        <span className="rounded bg-amber-500/20 px-1.5 py-0.5 font-mono text-xs text-amber-300">
          {log.action}
        </span>
        {log.target_type && (
          <span className="text-xs text-neutral-400">
            {log.target_type}
            {log.target_id && (
              <span className="ml-1 font-mono text-neutral-300">
                {log.target_id}
              </span>
            )}
          </span>
        )}
        <span className="text-xs text-neutral-400">
          実行者{" "}
          <span className="font-mono text-neutral-300">
            {log.actor_discord_user_id}
          </span>
        </span>
        <span className="ml-auto text-xs text-neutral-500">
          #{log.id} · {formatDateTime(log.created_at)}
        </span>
      </div>
      {hasSnapshot && (
        <details className="mt-2">
          <summary className="cursor-pointer text-xs text-neutral-400 hover:text-neutral-200">
            変更前後・詳細を見る
          </summary>
          <div className="mt-2 grid gap-2 lg:grid-cols-3">
            <Snapshot label="before" value={log.before} />
            <Snapshot label="after" value={log.after} />
            <Snapshot label="detail" value={log.detail} />
          </div>
        </details>
      )}
    </li>
  );
}

function Snapshot({ label, value }: { label: string; value: unknown }) {
  if (value === null || value === undefined) return null;
  return (
    <div>
      <p className="mb-1 text-xs text-neutral-500">{label}</p>
      <pre className="max-h-64 overflow-auto rounded-md bg-black/40 p-2 font-mono text-xs whitespace-pre-wrap text-neutral-300">
        {JSON.stringify(value, null, 2)}
      </pre>
    </div>
  );
}
