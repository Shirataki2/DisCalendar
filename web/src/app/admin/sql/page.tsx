import type { Metadata } from "next";
import { AdminOps } from "@/components/admin-ops";
import { AdminSqlConsole } from "@/components/admin-sql-console";

export const metadata: Metadata = {
  title: "SQL コンソール | 管理コンソール",
};

/**
 * 読み取り専用 SQL コンソールと定型操作 (#36)。管理者の確認は admin/layout.tsx、
 * 実行の制約と認可は api (`POST /admin/sql`, `POST /admin/ops/*`) 側
 */
export default function AdminSqlPage() {
  return (
    <main className="flex-1 overflow-y-auto p-8">
      <h1 className="mb-6 text-xl font-bold">SQL コンソール・定型操作</h1>
      <div className="flex flex-col gap-10">
        <AdminSqlConsole />
        <AdminOps />
      </div>
    </main>
  );
}
