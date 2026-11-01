import { formatPercent, formatYearMonth } from "@/lib/admin-format";
import type {
  AdminGuildRetention,
  AdminRetentionCohort,
} from "@/lib/api/types";

/** 表に最初から出す導入月の数。それより古い月は折りたたむ */
const VISIBLE_MONTHS = 12;

/** 割合と、その分子 / 分母 */
function Rate({ value, total }: { value: number; total: number }) {
  return (
    <span className="flex flex-col items-end">
      <span className="tabular-nums">{formatPercent(value, total)}</span>
      <span className="text-[10px] text-neutral-500 tabular-nums">
        {`${value.toLocaleString()} / ${total.toLocaleString()}`}
      </span>
    </span>
  );
}

function Rows({ cohorts }: { cohorts: AdminRetentionCohort[] }) {
  return cohorts.map((cohort) => (
    <tr key={cohort.month} className="border-t border-white/5">
      <td className="px-2 py-1 tabular-nums">
        {formatYearMonth(cohort.month)}
      </td>
      <td className="px-2 py-1 text-right tabular-nums">
        {cohort.guilds.toLocaleString()}
      </td>
      <td className="px-2 py-1 text-right">
        <Rate value={cohort.week_created} total={cohort.week_eligible} />
      </td>
      <td className="px-2 py-1 text-right">
        <Rate value={cohort.month_created} total={cohort.month_eligible} />
      </td>
      <td className="px-2 py-1 text-right">
        <Rate value={cohort.active} total={cohort.guilds} />
      </td>
    </tr>
  ));
}

function Head({ recentDays }: { recentDays: number }) {
  return (
    <thead className="text-xs text-neutral-400">
      <tr>
        <th className="px-2 py-1 text-left font-normal">導入月</th>
        <th className="px-2 py-1 text-right font-normal">導入 (参加中)</th>
        <th className="px-2 py-1 text-right font-normal">7 日以内に作成</th>
        <th className="px-2 py-1 text-right font-normal">30 日以内に作成</th>
        <th className="px-2 py-1 text-right font-normal">
          直近 {recentDays} 日も作成
        </th>
      </tr>
    </thead>
  );
}

/**
 * 導入月ごとの定着状況 (#375)。導入から最初の予定作成、使い続けているかまでを月ごとに並べる。
 * 定義と精度の限界は api/src/models/guild_retention.rs
 */
export function RetentionTable({
  retention,
  recentDays,
}: {
  retention: AdminGuildRetention;
  recentDays: number;
}) {
  const unknown = retention.unknown_joined > 0 && (
    <p className="text-xs text-neutral-500">
      {`導入日が分からない参加中のサーバーが ${retention.unknown_joined.toLocaleString()} 件あり、表には含めていない (うち直近 ${recentDays} 日に予定を作ったのは ${retention.unknown_joined_active.toLocaleString()} 件)`}
    </p>
  );
  // 導入日がすべて不明 (backfill 前など) でも、不明の件数と直近の利用は出す
  if (retention.cohorts.length === 0) {
    return (
      <div className="flex flex-col gap-2">
        <p className="py-2 text-sm text-neutral-500">
          導入日の分かる参加中のサーバーがありません
        </p>
        {unknown}
      </div>
    );
  }
  const visible = retention.cohorts.slice(0, VISIBLE_MONTHS);
  const older = retention.cohorts.slice(VISIBLE_MONTHS);
  return (
    <div className="flex flex-col gap-2">
      <div className="overflow-x-auto">
        <table className="w-full min-w-[32rem] text-sm">
          <Head recentDays={recentDays} />
          <tbody>
            <Rows cohorts={visible} />
          </tbody>
        </table>
      </div>
      {older.length > 0 && (
        <details>
          <summary className="cursor-pointer text-xs text-neutral-400">
            {`それより前の ${older.length} か月 (導入日は推定値)`}
          </summary>
          <div className="overflow-x-auto">
            <table className="w-full min-w-[32rem] text-sm">
              <Head recentDays={recentDays} />
              <tbody>
                <Rows cohorts={older} />
              </tbody>
            </table>
          </div>
        </details>
      )}
      {unknown}
    </div>
  );
}
