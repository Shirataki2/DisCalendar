import type { ReactNode } from "react";
import { formatChange, formatPercent } from "@/lib/admin-format";
import type { AdminTrend } from "@/lib/api/types";
import { cn } from "@/lib/utils";

/**
 * 分析情報 (#79) の表示部品。グラフはライブラリを足さず CSS だけで描く
 * (管理者しか開かないページのために依存とバンドルを増やさない)。
 *
 * どのグラフも 1 系列なので凡例は置かず、見出しが系列名を兼ねる。
 * 系列を色で区別する必要が無いので、データの色は下の 1 色だけを使う
 * (緑・黄・赤は管理コンソールでは状態を表す色なので、データには使わない)。
 */

/** データの色。暗い面 (#1e1e1e) の上でのコントラストと彩度を確認済み */
const ACCENT = "#6366f1";

/** 値が 0 でない棒を必ず見えるようにする最低の高さ (%) */
const MIN_BAR_HEIGHT = 2;

/** グラフの 1 点 */
export type ChartPoint = {
  /** x 軸のラベル (`8/25` など) */
  label: string;
  value: number;
};

export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3">
      <div>
        <h2 className="text-base font-semibold">{title}</h2>
        {description && (
          <p className="mt-1 text-xs text-neutral-400">{description}</p>
        )}
      </div>
      {children}
    </section>
  );
}

export function Card({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        "rounded-lg border border-white/10 bg-white/5 p-4",
        className,
      )}
    >
      {children}
    </div>
  );
}

/**
 * 期間の件数と、その直前の同じ長さの期間との比較。
 * 増減は色だけでなく矢印と「前の◯◯」の実数でも示す (色だけに意味を持たせない)
 */
export function TrendStat({
  label,
  trend,
  unit,
  note,
  previousLabel,
}: {
  label: string;
  trend: AdminTrend;
  /** 「人」「件」など */
  unit: string;
  note?: ReactNode;
  /** 比較対象の呼び方 (「前日」「前週」など) */
  previousLabel: string;
}) {
  const rising = trend.delta > 0;
  const flat = trend.delta === 0;
  return (
    <Card>
      <p className="text-xs text-neutral-400">{label}</p>
      <p className="mt-1 text-2xl font-bold tabular-nums">
        {trend.current.toLocaleString()}
        <span className="ml-1 text-sm font-normal text-neutral-400">
          {unit}
        </span>
      </p>
      <p className="mt-1 flex flex-wrap items-baseline gap-x-2 text-xs">
        <span
          className={cn(
            "tabular-nums",
            flat && "text-neutral-400",
            !flat && (rising ? "text-emerald-300" : "text-orange-300"),
          )}
        >
          {flat ? "±" : rising ? "▲" : "▼"}
          {Math.abs(trend.delta).toLocaleString()}
          {/* 前の期間が 0 だと増減率は出せないので、件数だけ見せる */}
          {trend.change_percent !== null &&
            ` (${formatChange(trend.change_percent)})`}
        </span>
        <span className="text-neutral-500">
          {previousLabel} {trend.previous.toLocaleString()}
          {unit}
        </span>
      </p>
      {note && <p className="mt-1 text-xs text-neutral-500">{note}</p>}
    </Card>
  );
}

/** x 軸の最後のラベルとの間に空けておく本数 */
const LABEL_GAP = 2;

/**
 * x 軸のラベルを出す位置か。最後の 1 本は必ず出し、そこから [`LABEL_GAP`] 本ぶん手前までは
 * `labelStride` の位置でも出さない (「8/24 8/25」のように隣り合って重なるのを防ぐ)
 */
function showLabelAt(index: number, count: number, labelStride: number) {
  if (index === count - 1) return true;
  return index % labelStride === 0 && count - 1 - index > LABEL_GAP;
}

/**
 * 推移の棒グラフ。`points` は古い順。
 *
 * - x 軸のラベルは `labelStride` 本おき (と最後の 1 本) だけ出す
 * - 数値を直接置くのは最大値の目安線だけにして、棒に数字を並べない
 * - 各棒の実数はマウスオーバー (title 属性) と下の表で確認できる
 */
export function BarChart({
  title,
  unit,
  points,
  labelStride,
  summary,
}: {
  title: string;
  unit: string;
  points: ChartPoint[];
  labelStride: number;
  /** 見出しの右に出す補足 (合計など) */
  summary?: ReactNode;
}) {
  const max = points.reduce((acc, point) => Math.max(acc, point.value), 0);
  const total = points.reduce((acc, point) => acc + point.value, 0);
  const peak = points.find((point) => point.value === max);
  const first = points.at(0);
  const last = points.at(-1);

  return (
    <Card className="flex flex-col gap-3">
      <div className="flex flex-wrap items-baseline justify-between gap-x-3">
        <h3 className="text-sm font-medium">{title}</h3>
        {summary && <p className="text-xs text-neutral-400">{summary}</p>}
      </div>
      {max === 0 ? (
        <p className="py-6 text-center text-sm text-neutral-500">
          この期間の記録はありません
        </p>
      ) : (
        <div
          role="img"
          aria-label={`${title}の推移。${first?.label} から ${last?.label} まで、合計 ${total.toLocaleString()}${unit}、最大は ${peak?.label} の ${max.toLocaleString()}${unit}`}
        >
          {/* 最大値の目安線。目盛りは主役ではないので細く薄く */}
          <div className="relative flex h-32 items-end gap-[2px] border-t border-dashed border-white/10">
            <span className="-top-2 absolute right-0 bg-[#1e1e1e] pl-1 text-[10px] text-neutral-500 tabular-nums">
              {max.toLocaleString()}
              {unit}
            </span>
            {points.map((point) => (
              <div
                key={point.label}
                title={`${point.label}: ${point.value.toLocaleString()}${unit}`}
                className="flex h-full flex-1 items-end justify-center"
              >
                <div
                  style={{
                    height: `${point.value === 0 ? 0 : Math.max(MIN_BAR_HEIGHT, (point.value / max) * 100)}%`,
                    backgroundColor: ACCENT,
                  }}
                  // 点が少ないグラフ (月別など) で棒が太い板にならないよう上限を付ける
                  className="w-full max-w-10 rounded-t-[4px]"
                />
              </div>
            ))}
          </div>
          <div className="mt-1 flex gap-[2px] text-[10px] text-neutral-500">
            {points.map((point, index) => (
              // ラベルは棒より幅が広いので、切り詰めずに空の枠へはみ出させる
              <span
                key={point.label}
                className="flex-1 whitespace-nowrap text-center tabular-nums"
              >
                {showLabelAt(index, points.length, labelStride)
                  ? point.label
                  : ""}
              </span>
            ))}
          </div>
        </div>
      )}
      <details className="text-xs text-neutral-400">
        <summary className="cursor-pointer select-none hover:text-white">
          数値を表で見る
        </summary>
        <div className="mt-2 max-h-56 overflow-y-auto">
          <table className="w-full">
            <thead className="sticky top-0 bg-[#1e1e1e] text-left text-neutral-500">
              <tr>
                <th scope="col" className="py-1 font-normal">
                  期間
                </th>
                <th scope="col" className="py-1 text-right font-normal">
                  {title} ({unit})
                </th>
              </tr>
            </thead>
            <tbody>
              {points.map((point) => (
                <tr key={point.label} className="border-white/5 border-t">
                  <th scope="row" className="py-1 font-normal tabular-nums">
                    {point.label}
                  </th>
                  <td className="py-1 text-right tabular-nums">
                    {point.value.toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </details>
    </Card>
  );
}

/** 全体に占める割合を 1 本の帯で表す (終日予定の割合など) */
export function ProportionBar({
  label,
  value,
  total,
  note,
}: {
  label: string;
  value: number;
  total: number;
  note?: ReactNode;
}) {
  const width = total === 0 ? 0 : (value / total) * 100;
  return (
    <div>
      <div className="flex flex-wrap items-baseline justify-between gap-x-2 text-sm">
        <span>{label}</span>
        <span className="tabular-nums">
          {value.toLocaleString()} / {total.toLocaleString()}
          <span className="ml-2 text-neutral-400">
            {formatPercent(value, total)}
          </span>
        </span>
      </div>
      <div
        className="mt-1 h-1.5 w-full overflow-hidden rounded-full bg-white/10"
        role="img"
        aria-label={`${label} は全体 ${total.toLocaleString()} のうち ${value.toLocaleString()} (${formatPercent(value, total)})`}
      >
        <div
          style={{ width: `${width}%`, backgroundColor: ACCENT }}
          className="h-full rounded-full"
        />
      </div>
      {note && <p className="mt-1 text-xs text-neutral-500">{note}</p>}
    </div>
  );
}

/** 順位付きの横棒 (予定の作成数が多いギルドなど)。行そのものが値の大きさを表す */
export function RankedBar({
  value,
  max,
  children,
}: {
  value: number;
  max: number;
  children: ReactNode;
}) {
  const width = max === 0 ? 0 : (value / max) * 100;
  return (
    <div className="relative isolate overflow-hidden rounded-md">
      <div
        aria-hidden="true"
        style={{ width: `${width}%`, backgroundColor: ACCENT }}
        className="-z-10 absolute inset-y-0 left-0 rounded-md opacity-25"
      />
      {children}
    </div>
  );
}
