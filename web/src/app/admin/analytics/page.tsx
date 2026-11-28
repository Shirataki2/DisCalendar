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
  RetentionTable,
  UserRetentionTable,
} from "@/components/admin-retention";
import {
  formatMonthDay,
  formatPercent,
  formatYearMonth,
} from "@/lib/admin-format";
import { serverApi } from "@/lib/api/server";
import type { AdminAnalytics } from "@/lib/api/types";
import { ROUTES } from "@/lib/site";

/** 招待リンクの導入元の表示名 (値は api/src/models/invite_clicks.rs の KNOWN_SOURCES) */
const INVITE_SOURCE_LABELS: Record<string, string> = {
  "lp-hero": "トップページ (先頭)",
  "lp-footer": "トップページ (末尾)",
  "docs-start": "使い方: はじめに",
  "docs-invite": "使い方: Bot の招待",
  tutorial: "チュートリアル",
  "usecase-trpg": "用途別: TRPG・マーダーミステリー",
  "usecase-clan": "用途別: ゲームのクラン・チーム",
  topgg: "top.gg",
  discordbotsgg: "discord.bots.gg",
  x: "X",
  bot: "Bot のコマンド",
  direct: "導入元なし",
  other: "その他 (知らない値)",
};

/** 予定の作成元の表示名 (値は shared/event_history.rs の Source) */
const CREATION_SOURCE_LABELS: Record<string, string> = {
  web: "Web (画面・iCalendar の取り込み)",
  bot: "Bot のコマンド",
  admin: "管理コンソール",
  mcp: "AI アシスタント (MCP)",
  api_token: "API トークン",
};

export const metadata: Metadata = {
  title: "分析情報 | 管理コンソール",
};

/**
 * 分析情報 (#79)。概要 (`/admin`) が「今の件数」なのに対して、こちらは時間軸の指標を出す。
 *
 * アクティブユーザーは利用の記録 (#81) から数える。セッションからの推定は、期限切れセッションの
 * 定期削除 (#44) で過去の値がさかのぼって減るようになったので廃止した (#398)。定義と精度の限界は api 側
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
    event_creation,
    creation_sources: creationSources,
    breakdown,
    guilds,
    invite_clicks: inviteClicks,
    daily,
    monthly,
  } = analytics;
  const inviteMax = Math.max(
    0,
    ...inviteClicks.sources.map((source) => source.recent),
  );
  const creationSourceMax = Math.max(
    0,
    ...creationSources.map((source) => source.count),
  );
  const creationSourceTotal = creationSources.reduce(
    (acc, source) => acc + source.count,
    0,
  );
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

  const dailyActive: ChartPoint[] = daily.map((point) => ({
    label: dailyLabel(point.date),
    value: point.active_users,
  }));
  const dailyEvents: ChartPoint[] = daily.map((point) => ({
    label: dailyLabel(point.date),
    value: point.events,
  }));
  const dailyNewUsers: ChartPoint[] = daily.map((point) => ({
    label: dailyLabel(point.date),
    value: point.new_users,
  }));
  // 記録開始より前の月は 0 になるだけで意味が無いので、記録開始の月から並べる
  // (月初どうしの文字列比較。`2026-08-28` の月は `2026-08-01` 以降)
  const since = active_users.since;
  const recordedMonthly = since
    ? monthly.filter((point) => point.month >= `${since.slice(0, 7)}-01`)
    : [];
  const monthlyActiveUsers: ChartPoint[] = recordedMonthly.map((point) => ({
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

  const sum = (points: ChartPoint[]) =>
    points.reduce((acc, point) => acc + point.value, 0);

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <h1 className="mb-2 text-xl font-bold">分析情報</h1>
      <p className="mb-6 text-sm text-neutral-400">
        {`${formatMonthDay(analytics.today)} (JST) 時点。アクティブユーザーと新規ユーザーの定着は利用の記録 (#81${since ? `。${formatMonthDay(since)} から` : ""}) から、予定の数は作成日時と変更履歴から数えている。読み方は`}
        <a href="#notes" className="mx-1 underline">
          ページ下の注記
        </a>
        を確認する
      </p>
      <div className="flex flex-col gap-10">
        <Section
          title="アクティブユーザー"
          description={`Discord のサーバー参加者ではなく、web にログインして使った人の数。利用の記録 (1 人 1 日 1 行) から数え、期間は JST の日付で区切って今日 (まだ途中) を含む。${since ? `記録は ${formatMonthDay(since)} から` : "利用の記録はまだ無い (記録を入れた日から積み上がる)"}`}
        >
          <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <li>
              <TrendStat
                label="DAU (今日)"
                trend={active_users.daily}
                unit="人"
                previousLabel="昨日は"
              />
            </li>
            <li>
              <TrendStat
                label="WAU (今日までの 7 日)"
                trend={active_users.weekly}
                unit="人"
                previousLabel="その前の 7 日は"
              />
            </li>
            <li>
              <TrendStat
                label={`MAU (今日までの ${analytics.recent_days} 日)`}
                trend={active_users.monthly}
                unit="人"
                previousLabel={`その前の ${analytics.recent_days} 日は`}
              />
            </li>
            <li>
              <Card>
                <p className="text-xs text-neutral-400">DAU/MAU 比</p>
                <p className="mt-1 text-2xl font-bold tabular-nums">
                  {active_users.stickiness === null
                    ? "-"
                    : `${Number((active_users.stickiness * 100).toFixed(1))}%`}
                </p>
                <p className="mt-1 text-xs text-neutral-500">
                  {`昨日までの ${analytics.recent_days} 日の平均 DAU ÷ その期間の利用者数。高いほど同じ人が繰り返し使っている`}
                </p>
              </Card>
            </li>
          </ul>
          <div className="grid gap-3 lg:grid-cols-2">
            <BarChart
              title="日別のアクティブユーザー"
              unit="人"
              points={dailyActive}
              labelStride={DAILY_STRIDE}
              summary={`直近 ${analytics.daily_days} 日`}
              partialLast={PARTIAL_DAY}
            />
            <BarChart
              title="月別のアクティブユーザー (MAU)"
              unit="人"
              points={monthlyActiveUsers}
              labelStride={MONTHLY_STRIDE}
              summary={
                since
                  ? `暦月。記録開始の ${monthlyLabel(`${since.slice(0, 7)}-01`)} は ${formatMonthDay(since)} から`
                  : undefined
              }
              partialLast={PARTIAL_MONTH}
            />
          </div>
        </Section>

        <Section
          title="予定の作成"
          description="予定が作られた件数。削除した予定もゴミ箱にある 30 日間は数えるが、完全に消えると数えられなくなるため、過去ほど実際より少なく出る"
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
          <Card className="flex flex-col gap-2">
            <div className="flex flex-wrap items-baseline justify-between gap-x-3">
              <h3 className="text-sm font-medium">
                作成元 (直近 {analytics.recent_days} 日)
              </h3>
              <p className="text-xs text-neutral-400">
                {`変更履歴の「作成」${creationSourceTotal.toLocaleString()} 件の内訳`}
              </p>
            </div>
            {creationSources.length === 0 ? (
              <p className="py-2 text-sm text-neutral-500">
                この期間に予定は作られていません
              </p>
            ) : (
              <ol className="flex flex-col gap-1">
                {creationSources.map((source) => (
                  <li key={source.source}>
                    <RankedBar value={source.count} max={creationSourceMax}>
                      <span className="flex items-baseline gap-2 px-2 py-1 text-sm">
                        <span>
                          {CREATION_SOURCE_LABELS[source.source] ??
                            source.source}
                        </span>
                        <span className="ml-auto shrink-0 text-neutral-400 text-xs tabular-nums">
                          {formatPercent(source.count, creationSourceTotal)}
                        </span>
                        <span className="w-14 shrink-0 text-right tabular-nums">
                          {source.count.toLocaleString()} 件
                        </span>
                      </span>
                    </RankedBar>
                  </li>
                ))}
              </ol>
            )}
            <p className="text-xs text-neutral-500">
              予定の変更履歴 (180 日保持)
              から数えるので、上の作成数とは少しずれる。ゴミ箱から完全に消えた予定と、100
              回を超えて更新された予定の作成記録は残らない
            </p>
          </Card>
        </Section>

        <Section
          title="新規ユーザー"
          description="Better Auth に利用者が登録された数 (初回ログイン)。定着は、登録のあとにまた使いに来たかを登録月ごとに見る。登録日当日の利用は数えず、7 日・30 日の割合はその日数が過ぎ切った人だけを分母にしている"
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
              title="月別の新規ユーザー"
              unit="人"
              points={monthlyNewUsers}
              labelStride={MONTHLY_STRIDE}
              summary={`直近 ${analytics.monthly_months} ヶ月で ${sum(monthlyNewUsers).toLocaleString()} 人`}
              partialLast={PARTIAL_MONTH}
            />
          </div>
          <Card className="flex flex-col gap-2">
            <h3 className="text-sm font-medium">
              登録月ごとの定着
              {analytics.user_retention.since && (
                <span className="ml-2 font-normal text-neutral-400 text-xs">
                  {`利用の記録が始まった ${formatMonthDay(analytics.user_retention.since)} 以降に登録した人だけ`}
                </span>
              )}
            </h3>
            <UserRetentionTable
              retention={analytics.user_retention}
              recentDays={analytics.recent_days}
            />
          </Card>
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
          title="導入からの定着"
          description={`Bot を導入した月ごとに、導入後に予定が作られたかと、今も使われているかを見る (#375)。7 日・30 日の割合は、導入からその日数が経ったサーバーだけを分母にしている。退出したサーバーは数えられないため、導入数は「今も参加しているサーバー」で、割合は実際より高めに出る。2026-08-30 (#125) より前の導入日は運営ログからの推定値なので、古い月は参考値として見る`}
        >
          <Card>
            <RetentionTable
              retention={analytics.retention}
              recentDays={analytics.recent_days}
            />
          </Card>
        </Section>

        <Section
          title="招待リンクの導入元"
          description={`/invite?from=<導入元> が開かれた回数 (#374)。招待画面を開いた回数で、実際に Bot が導入されたかは分からない。Discord の App Directory やダッシュボードの「サーバーに追加」など /invite を通らない導入は数えていない${inviteClicks.since ? `。${formatMonthDay(inviteClicks.since)} から記録` : ""}`}
        >
          <Card className="flex flex-col gap-2">
            {inviteClicks.sources.length === 0 ? (
              <p className="py-2 text-sm text-neutral-500">
                まだ記録がありません
              </p>
            ) : (
              <table className="w-full text-sm">
                <thead className="text-xs text-neutral-400">
                  <tr>
                    <th className="px-2 py-1 text-left font-normal">導入元</th>
                    <th className="w-24 px-2 py-1 text-right font-normal">
                      直近 {analytics.recent_days} 日
                    </th>
                    <th className="w-24 px-2 py-1 text-right font-normal">
                      その前の {analytics.recent_days} 日
                    </th>
                    <th className="w-24 px-2 py-1 text-right font-normal">
                      合計
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {inviteClicks.sources.map((source) => (
                    <tr key={source.source}>
                      <td className="px-0 py-0.5">
                        <RankedBar value={source.recent} max={inviteMax}>
                          <span className="flex items-baseline gap-2 px-2 py-1">
                            <span>
                              {INVITE_SOURCE_LABELS[source.source] ??
                                source.source}
                            </span>
                            <span className="font-mono text-[10px] text-neutral-500">
                              {source.source}
                            </span>
                          </span>
                        </RankedBar>
                      </td>
                      <td className="px-2 text-right tabular-nums">
                        {source.recent.toLocaleString()}
                      </td>
                      <td className="px-2 text-right text-neutral-400 tabular-nums">
                        {source.previous.toLocaleString()}
                      </td>
                      <td className="px-2 text-right text-neutral-400 tabular-nums">
                        {source.total.toLocaleString()}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Card>
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
                アクティブユーザーと定着は、利用の記録を入れた日からしか無い。
              </strong>
              ログイン済みの利用者からのリクエストを api が受けた日を 1 人 1 日
              1 行で積み上げた値で、それより前にさかのぼっては取れない。
              運営者による管理コンソールの閲覧は数えない。セッション (ログイン)
              の記録は最後の利用から約 7 日で消える (#44)
              ので、分析には使っていない
            </li>
            <li>
              <strong className="font-medium text-neutral-300">
                削除されたデータは数えられない。
              </strong>
              予定はゴミ箱から完全に消えると数えられなくなり、退会した利用者は新規ユーザーにも定着にも入らないので、過去ほど実際より少なく出る
            </li>
            <li>
              予定の作成元は変更履歴から数える。履歴は 180 日で消え、履歴の記録
              (#165) より前の予定は数えられない
            </li>
            <li>
              推移の右端 (今日・当月)
              はまだ期間の途中なので、棒を薄くしてある。完成した期間と比べない
            </li>
            <li>
              ギルドの退出の日時は DB
              に無く、退出したギルドは行ごと消えるため、サーバー数の推移は出せない
              (導入からの定着も「今も参加しているサーバー」だけで数えている)
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
