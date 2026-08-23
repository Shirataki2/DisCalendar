"use client";

import { Loader2Icon, PlayIcon } from "lucide-react";
import { useCallback, useId, useState } from "react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ApiError, describeApiError } from "@/lib/api";
import {
  ADMIN_SQL_MAX_CELL_CHARS,
  ADMIN_SQL_MAX_CHARS,
  ADMIN_SQL_MAX_ROWS,
  ADMIN_SQL_TIMEOUT_SECONDS,
  type SqlHistoryEntry,
  type SqlResult,
} from "@/lib/api/types";
import { useRunSql, useSqlHistory } from "@/lib/query/admin-sql";

const PLACEHOLDER = `SELECT guild_id, name FROM guilds ORDER BY name LIMIT 20
-- Ctrl/⌘ + Enter で実行`;

/**
 * SQL コンソールのエラー表示。api が 400 で返す Postgres のメッセージと、
 * 503 (SQL コンソール用の DB ロールが無い) の案内はそのまま見せる
 */
function describeSqlError(error: unknown): string {
  if (
    error instanceof ApiError &&
    (error.kind === "bad_request" || error.kind === "unavailable")
  ) {
    return error.message;
  }
  return describeApiError(error);
}

function formatTime(iso: string): string {
  return new Date(iso).toLocaleString("ja-JP", {
    timeZone: "Asia/Tokyo",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/**
 * 読み取り専用 SQL コンソール (#36)。テキストエリアの SQL を `POST /admin/sql` で実行し、結果を表で出す。
 * 制約 (読み取り専用・タイムアウト・行数上限・保護テーブル) は api 側で強制されていて、ここは入出力だけ
 */
export function AdminSqlConsole() {
  const [sql, setSql] = useState("");
  const [result, setResult] = useState<SqlResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const runSql = useRunSql();
  const history = useSqlHistory();
  const textareaId = useId();

  const execute = useCallback(async () => {
    const text = sql.trim();
    if (!text || runSql.isPending) return;
    setError(null);
    try {
      setResult(await runSql.mutateAsync(text));
    } catch (err) {
      setResult(null);
      setError(describeSqlError(err));
    }
  }, [sql, runSql]);

  const onKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      void execute();
    }
  };

  const tooLong = sql.length > ADMIN_SQL_MAX_CHARS;

  return (
    <section aria-labelledby="sql-heading" className="flex flex-col gap-3">
      <div>
        <h2 id="sql-heading" className="text-base font-semibold">
          SQL コンソール (読み取り専用)
        </h2>
        <p className="mt-1 text-xs text-neutral-400">
          SELECT / WITH / VALUES / TABLE / EXPLAIN / SHOW の 1
          文だけ。読み取り専用の DB ロールと READ ONLY
          トランザクションで実行し、
          {ADMIN_SQL_TIMEOUT_SECONDS} 秒で打ち切り、先頭 {ADMIN_SQL_MAX_ROWS} 行
          (1 セル {ADMIN_SQL_MAX_CELL_CHARS.toLocaleString()} 文字)
          まで返す。Better Auth の account / session / verification (トークン類)
          は読めない。実行はすべて監査ログに残る
        </p>
      </div>
      <label htmlFor={textareaId} className="sr-only">
        SQL
      </label>
      <Textarea
        id={textareaId}
        value={sql}
        onChange={(event) => setSql(event.target.value)}
        onKeyDown={onKeyDown}
        placeholder={PLACEHOLDER}
        spellCheck={false}
        aria-invalid={tooLong || undefined}
        className="min-h-32 font-mono text-xs"
      />
      <div className="flex flex-wrap items-center gap-3">
        <Button
          type="button"
          size="sm"
          onClick={() => void execute()}
          disabled={!sql.trim() || tooLong || runSql.isPending}
        >
          {runSql.isPending ? (
            <Loader2Icon data-icon="inline-start" className="animate-spin" />
          ) : (
            <PlayIcon data-icon="inline-start" />
          )}
          実行
        </Button>
        <span className="text-xs text-neutral-500">
          {sql.length.toLocaleString()} / {ADMIN_SQL_MAX_CHARS.toLocaleString()}{" "}
          文字
        </span>
        {result && (
          <span className="text-xs text-neutral-400">
            {result.row_count.toLocaleString()} 行 ({result.duration_ms} ms)
            {result.truncated && (
              <span className="ml-2 text-amber-300">
                行数 ({ADMIN_SQL_MAX_ROWS}) かサイズの上限で打ち切りました
                (続きは LIMIT / OFFSET で)
              </span>
            )}
          </span>
        )}
      </div>
      {error && (
        <pre
          role="alert"
          className="overflow-x-auto rounded-md bg-red-900/40 px-3 py-2 font-mono text-xs whitespace-pre-wrap text-red-200"
        >
          {error}
        </pre>
      )}
      {result && <ResultTable result={result} />}
      <History
        entries={history.data}
        isError={history.isError}
        onPick={(entry) => setSql(entry.sql)}
      />
    </section>
  );
}

function ResultTable({ result }: { result: SqlResult }) {
  if (result.columns.length === 0) {
    return (
      <p className="text-xs text-neutral-400">この文は結果セットを返しません</p>
    );
  }
  return (
    <div className="max-h-[60vh] overflow-auto rounded-lg border border-white/10">
      <table className="w-max min-w-full text-xs">
        <thead className="sticky top-0 bg-neutral-900 text-left text-neutral-400">
          <tr>
            <th className="px-2 py-1.5 font-normal">#</th>
            {result.columns.map((column, i) => (
              // 同名のカラムがありうるので index をキーに含める
              <th key={`${i}-${column.name}`} className="px-2 py-1.5">
                <span className="font-medium text-neutral-200">
                  {column.name}
                </span>
                <span className="ml-1 font-normal text-neutral-500">
                  {column.type.toLowerCase()}
                </span>
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="font-mono">
          {result.rows.map((row, rowIndex) => (
            <tr
              // biome-ignore lint/suspicious/noArrayIndexKey: 結果セットの行に識別子は無く、並び替えもしない (位置がそのまま意味)
              key={rowIndex}
              className="border-t border-white/5 hover:bg-white/5"
            >
              <td className="px-2 py-1 text-neutral-500 tabular-nums">
                {rowIndex + 1}
              </td>
              {row.map((value, colIndex) => (
                <td
                  // biome-ignore lint/suspicious/noArrayIndexKey: 同上 (値はカラム位置で決まる)
                  key={colIndex}
                  className="max-w-md truncate px-2 py-1 align-top"
                  title={value ?? undefined}
                >
                  {value === null ? (
                    <span className="text-neutral-500 italic">NULL</span>
                  ) : (
                    value
                  )}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function History({
  entries,
  isError,
  onPick,
}: {
  entries: SqlHistoryEntry[] | undefined;
  isError: boolean;
  onPick: (entry: SqlHistoryEntry) => void;
}) {
  return (
    <div>
      <h3 className="mb-2 text-sm font-semibold">直近の実行履歴</h3>
      {isError ? (
        <p className="text-xs text-red-300">履歴を取得できませんでした</p>
      ) : !entries ? (
        <p className="text-xs text-neutral-500">読み込み中…</p>
      ) : entries.length === 0 ? (
        <p className="text-xs text-neutral-500">まだ実行履歴はありません</p>
      ) : (
        <ul className="divide-y divide-white/5 rounded-lg border border-white/10 text-xs">
          {entries.map((entry) => (
            <li key={entry.id}>
              <button
                type="button"
                onClick={() => onPick(entry)}
                title="クリックするとテキストエリアに入ります"
                className="flex w-full items-start gap-3 px-3 py-2 text-left hover:bg-white/5"
              >
                <span className="shrink-0 text-neutral-500 tabular-nums">
                  {formatTime(entry.created_at)}
                </span>
                <code className="min-w-0 flex-1 truncate font-mono text-neutral-200">
                  {entry.sql}
                </code>
                <span className="shrink-0 text-neutral-400">
                  {entry.error !== null ? (
                    <span className="text-red-300" title={entry.error}>
                      エラー
                    </span>
                  ) : (
                    <>
                      {entry.row_count?.toLocaleString() ?? "-"} 行
                      {entry.truncated && "+"} / {entry.duration_ms ?? "-"} ms
                    </>
                  )}
                </span>
                <span
                  className="shrink-0 font-mono text-neutral-500"
                  title="実行した管理者の Discord ユーザー ID"
                >
                  {entry.actor_discord_user_id}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
