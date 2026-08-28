import type { Metadata } from "next";
import Link from "next/link";
import {
  BarChart,
  Card,
  type ChartPoint,
  ProportionBar,
  RankedBar,
  Section,
  TrendStat,
} from "@/components/admin-chart";
import {
  formatMonthDay,
  formatPercent,
  formatYearMonth,
} from "@/lib/admin-format";
import { serverApi } from "@/lib/api/server";
import type { AdminAnalytics } from "@/lib/api/types";
import { ROUTES } from "@/lib/site";

export const metadata: Metadata = {
  title: "分析情報 | 管理コンソール",
};

/**
 * 分析情報 (#79)。概要 (`/admin`) が「今の件数」なのに対して、こちらは時間軸の指標を出す。
 *
 * アクティブユーザーは利用の記録 (#81) から数えた実測と、セッションからの推定の 2 系統を併記する
 * (実測は記録を入れた日からしか無いので、それ以前の期間は推定で見る)。それ以外の指標は
 * 既存データからの推定・集計のまま。定義と精度の限界は api 側
 * (`api/src/models/admin_analytics.rs`) に書いてあり、運用者が数字を誤読しないよう
 * このページにも同じ内容を注記する
 */
export default async function AdminAnalyticsPage() {
  let analytics: AdminAnalytics;
  try {
    analytics = await serverApi.admin.analytics();
  } catch {
    return (
      <main className="flex-1 overflow-y-auto p-8">
        <h1 className="mb-2 text-xl font-bold">分析情報</h1>
        <p
          role="alert"
          className="rounded-md bg-red-900/40 px-3 py-2 text-sm text-red-200"
        >
          集計 (GET /admin/analytics) の取得に失敗しました。api
          のログを確認してください
        </p>
      </main>
    );
  }

  const {
    active_users,
    measured_active_users: measured,
    event_creation,
    breakdown,
    guilds,
    daily,
    monthly,
  } = analytics;
  const dailyLabel = (date: string) => formatMonthDay(date);
  const monthlyLabel = (month: string) => formatYearMonth(month);
  // 日別は週の区切り、月別は四半期の区切りでラベルを出す
  const DAILY_STRIDE = 7;
  const MONTHLY_STRIDE = 3;
  // 推移の右端 (今日 / 当月) はまだ期間の途中なので、完成した期間と同列に読まれないよう注記する
  const PARTIAL_DAY = `右端の ${formatMonthDay(analytics.today)} は今日 (集計時点まで)`;
  const lastMonth = monthly.at(-1);
  const PARTIAL_MONTH = lastMonth
    ? `右端の ${monthlyLabel(lastMonth.month)} は当月の途中まで`
    : undefined;

  const dailyMeasuredActive: ChartPoint[] = daily.map((point) => ({
    label: dailyLabel(point.date),
    value: point.measured_active_users,
  }));
  const dailyEvents: ChartPoint[] = daily.map((point) => ({
    label: dailyLabel(point.date),
    value: point.events,
  }));
  const dailyNewUsers: ChartPoint[] = daily.map((point) => ({
    label: dailyLabel(point.date),
    value: point.new_users,
  }));
  const dailyLogins: ChartPoint[] = daily.map((point) => ({
    label: dailyLabel(point.date),
    value: point.logins,
  }));
  const monthlyActiveUsers: ChartPoint[] = monthly.map((point) => ({
    label: monthlyLabel(point.month),
    value: point.active_users,
  }));
  const monthlyEvents: ChartPoint[] = monthly.map((point) => ({
    label: monthlyLabel(point.month),
    value: point.events,
  }));
  const monthlyNewUsers: ChartPoint[] = monthly.map((point) => ({
    label: monthlyLabel(point.month),
    value: point.new_users,
  }));
  const monthlyLogins: ChartPoint[] = monthly.map((point) => ({
    label: monthlyLabel(point.month),
    value: point.logins,
  }));

  const sum = (points: ChartPoint[]) =>
    points.reduce((acc, point) => acc + point.value, 0);

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <h1 className="mb-2 text-xl font-bold">分析情報</h1>
      <p className="mb-6 text-sm text-neutral-400">
        {`${formatMonthDay(analytics.today)} (JST) 時点。アクティブユーザーは利用の記録からの実測 (#81。記録を入れた日から) と、セッションからの推定を併記している。それ以外の数字はセッションと予定の作成日時からの推定・集計になる。読み方は`}
        <a href="#notes" className="mx-1 underline">
          ページ下の注記
        </a>
        を確認する
      </p>
      <div className="flex flex-col gap-10">
        <Section
          title="アクティブユーザー"
          description="Discord のサーバー参加者ではなく、web にログインして使った人の数。実測は利用の記録 (1 人 1 日 1 行) から数えた正確な値、推定はセッションが生きていた期間を使っていた期間とみなしたおおよその値"
        >
          <div className="flex flex-col gap-2">
            <h3 className="text-sm font-medium">
              実測
              <span className="ml-2 font-normal text-neutral-400 text-xs">
                {measured.since
                  ? `${formatMonthDay(measured.since)} から積み上げた利用の記録による。期間は JST の日付で区切り、今日 (まだ途中) を含む`
                  : "利用の記録はまだ無い (記録を入れた日から積み上がる)"}
              </span>
            </h3>
            <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              <li>
                <TrendStat
                  label="DAU (今日)"
                  trend={measured.daily}
                  unit="人"
                  previousLabel="昨日は"
                />
              </li>
              <li>
                <TrendStat
                  label="WAU (今日までの 7 日)"
                  trend={measured.weekly}
                  unit="人"
                  previousLabel="その前の 7 日は"
                />
              </li>
              <li>
                <TrendStat
                  label={`MAU (今日までの ${analytics.recent_days} 日)`}
                  trend={measured.monthly}
                  unit="人"
                  previousLabel={`その前の ${analytics.recent_days} 日は`}
                />
              </li>
            </ul>
          </div>
          <div className="flex flex-col gap-2">
            <h3 className="text-sm font-medium">
              推定
              <span className="ml-2 font-normal text-neutral-400 text-xs">
                セッションから計算し直した値。実測の記録開始より前の期間と比べるときに見る
              </span>
            </h3>
            <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              <li>
                <TrendStat
                  label="DAU (直近 1 日)"
                  trend={active_users.daily}
                  unit="人"
                  previousLabel="その前の 1 日は"
                />
              </li>
              <li>
                <TrendStat
                  label="WAU (直近 7 日)"
                  trend={active_users.weekly}
                  unit="人"
                  previousLabel="その前の 7 日は"
                />
              </li>
              <li>
                <TrendStat
                  label={`MAU (直近 ${analytics.recent_days} 日)`}
                  trend={active_users.monthly}
                  unit="人"
                  previousLabel={`その前の ${analytics.recent_days} 日は`}
                />
              </li>
            </ul>
          </div>
          <div className="grid gap-3 lg:grid-cols-2">
            <BarChart
              title="日別のアクティブユーザー (実測)"
              unit="人"
              points={dailyMeasuredActive}
              labelStride={DAILY_STRIDE}
              summary={
                measured.since
                  ? `記録は ${formatMonthDay(measured.since)} から`
                  : undefined
              }
              partialLast={PARTIAL_DAY}
            />
            <BarChart
              title="月別のアクティブユーザー (推定)"
              unit="人"
              points={monthlyActiveUsers}
              labelStride={MONTHLY_STRIDE}
              summary={`直近 ${analytics.monthly_months} ヶ月 (暦月)`}
              partialLast={PARTIAL_MONTH}
            />
          </div>
        </Section>

        <Section
          title="予定の作成"
          description="予定が作られた件数。削除された予定は行ごと消えるため数えられず、過去ほど実際より少なく出る"
        >
          <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <li>
              <TrendStat
                label="直近 24 時間"
                trend={event_creation.last_day}
                unit="件"
                previousLabel="その前の 24 時間は"
              />
            </li>
            <li>
              <TrendStat
                label="直近 7 日"
                trend={event_creation.last_week}
                unit="件"
                previousLabel="その前の 7 日は"
              />
            </li>
            <li>
              <TrendStat
                label={`直近 ${analytics.recent_days} 日`}
                trend={event_creation.last_month}
                unit="件"
                previousLabel={`その前の ${analytics.recent_days} 日は`}
              />
            </li>
            <li>
              <Card>
                <p className="text-xs text-neutral-400">
                  今 DB にある予定の総数
                </p>
                <p className="mt-1 text-2xl font-bold tabular-nums">
                  {event_creation.total.toLocaleString()}
                  <span className="ml-1 text-sm font-normal text-neutral-400">
                    件
                  </span>
                </p>
                <p className="mt-1 text-xs text-neutral-500">
                  削除された予定は含まれない
                </p>
              </Card>
            </li>
          </ul>
          <div className="grid gap-3 lg:grid-cols-2">
            <BarChart
              title="日別の予定の作成数"
              unit="件"
              points={dailyEvents}
              labelStride={DAILY_STRIDE}
              summary={`直近 ${analytics.daily_days} 日で ${sum(dailyEvents).toLocaleString()} 件`}
              partialLast={PARTIAL_DAY}
            />
            <BarChart
              title="月別の予定の作成数"
              unit="件"
              points={monthlyEvents}
              labelStride={MONTHLY_STRIDE}
              summary={`直近 ${analytics.monthly_months} ヶ月で ${sum(monthlyEvents).toLocaleString()} 件`}
              partialLast={PARTIAL_MONTH}
            />
          </div>
        </Section>

        <Section
          title="新規ユーザーとログイン"
          description="新規ユーザーは Better Auth に利用者が登録された数 (初回ログイン)。ログインはセッションが新しく作られた回数で、同じ人が別の端末やブラウザから入るたびに増える。ログインは保存された履歴ではなく今残っているセッションから数え直しているため、セッションを消すとさかのぼって減る"
        >
          <div className="grid gap-3 lg:grid-cols-2">
            <BarChart
              title="日別の新規ユーザー"
              unit="人"
              points={dailyNewUsers}
              labelStride={DAILY_STRIDE}
              summary={`直近 ${analytics.daily_days} 日で ${sum(dailyNewUsers).toLocaleString()} 人`}
              partialLast={PARTIAL_DAY}
            />
            <BarChart
              title="日別のログイン"
              unit="回"
              points={dailyLogins}
              labelStride={DAILY_STRIDE}
              summary={`直近 ${analytics.daily_days} 日で ${sum(dailyLogins).toLocaleString()} 回`}
              partialLast={PARTIAL_DAY}
            />
          </div>
          <div className="grid gap-3 lg:grid-cols-2">
            <BarChart
              title="月別の新規ユーザー"
              unit="人"
              points={monthlyNewUsers}
              labelStride={MONTHLY_STRIDE}
              summary={`直近 ${analytics.monthly_months} ヶ月で ${sum(monthlyNewUsers).toLocaleString()} 人`}
              partialLast={PARTIAL_MONTH}
            />
            <BarChart
              title="月別のログイン"
              unit="回"
              points={monthlyLogins}
              labelStride={MONTHLY_STRIDE}
              summary={`直近 ${analytics.monthly_months} ヶ月で ${sum(monthlyLogins).toLocaleString()} 回`}
              partialLast={PARTIAL_MONTH}
            />
          </div>
        </Section>

        <Section
          title="ギルドの利用状況"
          description={
            <>
              直近 {analytics.recent_days} 日に予定が作られたギルドを
              「使われている」とみなす。ギルドの詳細は
              <Link href={ROUTES.adminGuilds} className="ml-1 underline">
                ギルド一覧
              </Link>
              から
            </>
          }
        >
          <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            <li>
              <Card>
                <p className="text-xs text-neutral-400">
                  予定が作られた参加中のギルド (直近 {analytics.recent_days} 日)
                </p>
                <p className="mt-1 text-2xl font-bold tabular-nums">
                  {guilds.active_guilds.toLocaleString()}
                  <span className="ml-1 text-sm font-normal text-neutral-400">
                    サーバー
                  </span>
                </p>
                {/* 分子は参加中のギルドだけ (api 側で分けてある)。退出済みに残った予定を混ぜると
                    「参加中のうちどれだけ使われているか」にならず、100% を超えることすらある */}
                <p className="mt-1 text-xs text-neutral-500">
                  {`Bot 参加中の ${guilds.joined_guilds.toLocaleString()} サーバーの ${formatPercent(guilds.active_guilds, guilds.joined_guilds)}`}
                  {guilds.active_left_guilds > 0 &&
                    `。ほかに退出済みのギルド ${guilds.active_left_guilds.toLocaleString()} 件にも、この期間に作られた予定が残っている`}
                </p>
              </Card>
            </li>
            <li className="sm:col-span-1 lg:col-span-2">
              <Card className="flex flex-col gap-2">
                <h3 className="text-sm font-medium">
                  予定の作成が多いギルド (直近 {analytics.recent_days} 日)
                </h3>
                {guilds.top_guilds.length === 0 ? (
                  <p className="py-2 text-sm text-neutral-500">
                    この期間に予定は作られていません
                  </p>
                ) : (
                  <ol className="flex flex-col gap-1">
                    {guilds.top_guilds.map((guild, index) => (
                      <li key={guild.guild_id}>
                        <RankedBar
                          value={guild.event_count}
                          max={guilds.top_guilds[0].event_count}
                        >
                          <Link
                            href={`${ROUTES.adminGuilds}/${guild.guild_id}`}
                            className="flex items-center gap-2 px-2 py-1 text-sm hover:bg-white/5"
                          >
                            <span className="w-4 shrink-0 text-right text-neutral-500 text-xs tabular-nums">
                              {index + 1}
                            </span>
                            {guild.avatar_url ? (
                              // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
                              <img
                                src={guild.avatar_url}
                                alt=""
                                className="h-5 w-5 shrink-0 rounded-full"
                              />
                            ) : (
                              <span
                                aria-hidden="true"
                                className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-white/10 text-[10px]"
                              >
                                {(guild.name ?? "?").slice(0, 1)}
                              </span>
                            )}
                            <span className="truncate">
                              {guild.name ?? (
                                <span className="text-neutral-500">
                                  (退出済み)
                                </span>
                              )}
                            </span>
                            <span className="ml-auto shrink-0 font-mono text-[10px] text-neutral-500">
                              {guild.guild_id}
                            </span>
                            <span className="w-14 shrink-0 text-right tabular-nums">
                              {guild.event_count.toLocaleString()} 件
                            </span>
                          </Link>
                        </RankedBar>
                      </li>
                    ))}
                  </ol>
                )}
              </Card>
            </li>
          </ul>
        </Section>

        <Section
          title="予定と設定の内訳"
          description="今 DB にあるデータの内訳。推移ではなく現時点のスナップショット"
        >
          <div className="grid gap-3 lg:grid-cols-2">
            <Card className="flex flex-col gap-4">
              <h3 className="text-sm font-medium">予定</h3>
              <ProportionBar
                label="終日予定"
                value={breakdown.all_day_events}
                total={event_creation.total}
              />
              <ProportionBar
                label="開始時刻より前にも通知が届く予定"
                value={breakdown.events_with_notifications}
                total={event_creation.total}
                note={`予定 1 件あたり平均 ${breakdown.notifications_per_event.toFixed(2)} 件。Bot が必ず送る開始時刻の通知、解釈できない旧データの設定、発火時刻が重なる設定 (「60 分前」と「1 時間前」など。Bot は 1 回にまとめる) は数えていない`}
              />
            </Card>
            <Card className="flex flex-col gap-4">
              <h3 className="text-sm font-medium">
                ギルドの設定 (Bot 参加中の{" "}
                {guilds.joined_guilds.toLocaleString()} サーバー)
              </h3>
              <ProportionBar
                label="通知先チャンネルの ID が正しい形式"
                value={breakdown.guilds_with_channel}
                total={guilds.joined_guilds}
                note="旧データの 0 のような不正な値は除いてある。そのチャンネルが今も存在するか、Bot に送信権限があるかまでは確認していない (実際に届くかはギルド一覧の差分検出や Bot のログで見る)"
              />
              <ProportionBar
                label="restricted モード"
                value={breakdown.restricted_guilds}
                total={guilds.joined_guilds}
                note="サーバーの管理権限を持つ人だけが予定を編集できる設定"
              />
            </Card>
          </div>
        </Section>

        <section id="notes" className="scroll-mt-4">
          <h2 className="text-base font-semibold">数字の読み方と限界</h2>
          <ul className="mt-2 flex list-disc flex-col gap-1 pl-5 text-sm text-neutral-400">
            <li>
              <strong className="font-medium text-neutral-300">
                実測のアクティブユーザーは、記録を入れた日からしか無い。
              </strong>
              ログイン済みの利用者からのリクエストを api が受けた日を 1 人 1 日
              1 行で積み上げた値で、それより前にさかのぼっては取れない。
              記録開始より前の期間や、記録開始をまたぐ「前の期間との比較」は推定で見る。
              運営者による管理コンソールの閲覧は実測に数えない
            </li>
            <li>
              <strong className="font-medium text-neutral-300">
                削除されたデータは数えられない。
              </strong>
              予定は削除すると行ごと消えるので、過去の作成数は実際より少なく出る
            </li>
            <li>
              <strong className="font-medium text-neutral-300">
                セッションを消すと、推定のアクティブユーザーとログイン回数がさかのぼって減る。
              </strong>
              推定もログインも、保存された履歴ではなく今残っているセッションから
              計算し直している。
              <Link href={ROUTES.adminSql} className="mx-1 underline">
                定型操作
              </Link>
              の「期限切れセッションの削除」だけでなく、
              <Link href={ROUTES.adminUsers} className="mx-1 underline">
                ユーザー管理
              </Link>
              の「強制ログアウト」(期限内のセッションも消える)
              でも減るので、実行した時期より前の値は信用できない
              (実測はセッションを消しても減らない)
            </li>
            <li>
              推定のアクティブユーザーはセッションの生存期間で判定している。ログインしたまま使っていない場合も
              「使っている」と数えるため、実際よりやや多く出る。逆に、セッションの最終利用日時を更新するのは
              web
              のページ表示だけなので、画面を開いたまま予定の作成やカレンダーの再取得だけを続けている利用者は
              翌日以降 DAU から漏れる。実測にはどちらの問題もない
            </li>
            <li>
              推定はセッションの更新間隔 (最短 1 日)
              より細かい粒度が出せず、同じ日の再訪も数えない。実測と推定で期間の区切りも違う
              (実測は JST の日付、推定は集計時点からの 24 時間刻み) ため、DAU
              どうしは一致しないことがある
            </li>
            <li>
              推移の右端 (今日・当月)
              はまだ期間の途中なので、棒を薄くしてある。完成した期間と比べない
            </li>
            <li>
              ギルドの参加・退出の日時は DB
              に無いため、サーバー数の推移は出せない
            </li>
            <li>
              日付の区切りはすべて JST。月別は暦月 (1 日 0:00 〜 翌月 1 日 0:00)
              で区切る
            </li>
          </ul>
        </section>
      </div>
    </main>
  );
}
