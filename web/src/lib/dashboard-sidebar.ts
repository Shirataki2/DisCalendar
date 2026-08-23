// ダッシュボードのナビゲーションサイドバー (PC) の開閉状態を覚える cookie。
// サーバー側 (app/dashboard/layout.tsx) が初期状態を決め、クライアント側 (DashboardShell) が切り替え時に書く。
// サーバーとクライアントの両方から import するので "use client" のモジュールには置かない

export const SIDEBAR_COOKIE = "dashboard_sidebar";

/** cookie の値から初期状態を決める。未設定 (初回) は旧版と同じく開いた状態 */
export function isSidebarOpen(value: string | undefined): boolean {
  return value !== "closed";
}
