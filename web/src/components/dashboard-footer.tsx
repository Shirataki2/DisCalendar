import pkg from "../../package.json";

/**
 * ダッシュボードの固定フッタ (旧実装の AppFooter.vue 相当: 左にバージョン、右に © 表記)。
 * layout の h-dvh の最下段に置くので、本文がスクロールしても画面下に留まる。
 * バージョンは web/package.json の version (api / bot の Cargo.toml と揃えてある)
 */
export function DashboardFooter() {
  return (
    <footer className="flex h-8 shrink-0 items-center justify-between border-t border-white/10 px-4 text-xs text-neutral-400">
      <span>v {pkg.version}</span>
      <span>&copy; 2021 DisCalendar</span>
    </footer>
  );
}
