import type { Metadata } from "next";
import Link from "next/link";
import type { ReactNode } from "react";
import { getAdminMe } from "@/lib/admin";
import {
  formatDateTime,
  formatDuration,
  formatNaiveJst,
} from "@/lib/admin-format";
import { serverApi } from "@/lib/api/server";
import type {
  AdminMigrationStatus,
  AdminStats,
  AdminStatus,
} from "@/lib/api/types";
import { ROUTES } from "@/lib/site";

export const metadata: Metadata = {
  title: "管理コンソール",
};

/**
 * 管理コンソールの概要 (#37)。運用で最初に見る件数と、api / DB / マイグレーション / ビルドの状態。
 * どちらか一方の取得に失敗しても残りは見せたいので `allSettled` で受ける
 * (この画面自体が「調子が悪いときに開く画面」なので、落ちるのは避ける)
 */
export default async function AdminPage() {
  // layout で管理者であることは確認済み (非管理者は 404)
  const [admin, [statsResult, statusResult]] = await Promise.all([
    getAdminMe(),
    Promise.allSettled([serverApi.admin.stats(), serverApi.admin.status()]),
  ]);

  return (
    <main className="flex-1 overflow-y-auto p-8">
      <h1 className="mb-2 text-xl font-bold">概要</h1>
      <p className="mb-6 text-sm text-neutral-400">
        {admin?.name} (Discord ID: {admin?.discord_user_id})
        として管理者権限でログインしています。ここでの書き込み操作はすべて
        <Link href={ROUTES.adminAuditLogs} className="mx-1 underline">
          監査ログ
        </Link>
        に記録されます。
      </p>
      <div className="flex flex-col gap-8">
        {statsResult.status === "fulfilled" ? (
          <StatsSection stats={statsResult.value} />
        ) : (
          <FailedSection label="件数 (GET /admin/stats)" />
        )}
        {statusResult.status === "fulfilled" ? (
          <StatusSection status={statusResult.value} />
        ) : (
          <FailedSection label="稼働状況 (GET /admin/status)" />
        )}
        {statsResult.status === "fulfilled" && (
          <GuildActivitySection stats={statsResult.value} />
        )}
      </div>
    </main>
  );
}

function FailedSection({ label }: { label: string }) {
  return (
    <p
      role="alert"
      className="rounded-md bg-red-900/40 px-3 py-2 text-sm text-red-200"
    >
      {label} の取得に失敗しました。api のログを確認してください
    </p>
  );
}

function Section({
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

function Stat({
  label,
  value,
  note,
  href,
}: {
  label: string;
  value: number;
  note?: ReactNode;
  href?: string;
}) {
  const body = (
    <>
      <p className="text-xs text-neutral-400">{label}</p>
      <p className="mt-1 text-2xl font-bold tabular-nums">
        {value.toLocaleString()}
      </p>
      {note && <p className="mt-1 text-xs text-neutral-500">{note}</p>}
    </>
  );
  const className =
    "block rounded-lg border border-white/10 bg-white/5 p-4 transition-colors";
  return href ? (
    <Link href={href} className={`${className} hover:bg-white/10`}>
      {body}
    </Link>
  ) : (
    <div className={className}>{body}</div>
  );
}

function StatsSection({ stats }: { stats: AdminStats }) {
  const { counts } = stats;
  return (
    <Section title="件数">
      <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <li>
          <Stat
            label="ギルド (Bot 参加中)"
            value={counts.guilds}
            note={
              counts.left_guilds > 0
                ? `+ 退出済みでデータが残っているもの ${counts.left_guilds.toLocaleString()} 件`
                : "退出済みのデータは残っていない"
            }
            href={ROUTES.adminGuilds}
          />
        </li>
        <li>
          <Stat
            label="予定"
            value={counts.events}
            note={`これからの予定 ${counts.upcoming_events.toLocaleString()} 件`}
          />
        </li>
        <li>
          <Stat
            label="ユーザー"
            value={counts.users}
            note={`ログイン中のセッション ${counts.active_sessions.toLocaleString()} (期限切れ含め ${counts.sessions.toLocaleString()})`}
            href={ROUTES.adminUsers}
          />
        </li>
        <li>
          <Stat
            label="今日の通知予定"
            value={stats.notifications_today}
            note={`${formatNaiveJst(stats.day_start)} からの 24 時間 (JST) に Bot が送る通知の数`}
          />
        </li>
      </ul>
    </Section>
  );
}

function StatusSection({ status }: { status: AdminStatus }) {
  const { build, database, migrations } = status;
  return (
    <Section title="稼働状況">
      <dl className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        <Row label="api のバージョン">
          {build.version}
          {build.debug && (
            <span className="ml-2 rounded bg-amber-500/20 px-1.5 py-0.5 text-[10px] text-amber-300">
              debug ビルド
            </span>
          )}
        </Row>
        <Row label="コミット (GIT_SHA)">
          {build.git_sha ? (
            <span className="font-mono text-xs">
              {build.git_sha.slice(0, 12)}
            </span>
          ) : (
            <Unknown />
          )}
        </Row>
        <Row label="イメージタグ (IMAGE_TAG)">
          {build.image_tag ? (
            <span className="font-mono text-xs">{build.image_tag}</span>
          ) : (
            <Unknown />
          )}
        </Row>
        <Row label="起動">
          {formatDateTime(status.started_at)}
          <span className="ml-2 text-neutral-400">
            ({formatDuration(status.uptime_seconds)} 稼働)
          </span>
        </Row>
        <Row label="DB 接続">
          {database.reachable ? (
            <>
              <Badge tone="ok">接続できる</Badge>
              <span className="ml-2 text-neutral-400">
                {database.latency_ms} ms / PostgreSQL{" "}
                {database.server_version ?? "?"}
              </span>
            </>
          ) : (
            <>
              <Badge tone="error">接続できない</Badge>
              <span className="ml-2 text-neutral-400">{database.error}</span>
            </>
          )}
        </Row>
        <Row label="接続プール">
          {database.pool_connections} 接続 (待機 {database.pool_idle})
        </Row>
      </dl>
      <MigrationCard migrations={migrations} ok={status.migrations_ok} />
    </Section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="rounded-lg border border-white/10 bg-white/5 p-4">
      <dt className="text-xs text-neutral-400">{label}</dt>
      <dd className="mt-1 text-sm">{children}</dd>
    </div>
  );
}

function Unknown() {
  return (
    <span
      className="text-neutral-500"
      title="ビルド時に指定されていない (ローカルの cargo run など)"
    >
      不明
    </span>
  );
}

function Badge({
  tone,
  children,
}: {
  tone: "ok" | "warn" | "error";
  children: ReactNode;
}) {
  const className = {
    ok: "bg-emerald-500/20 text-emerald-300",
    warn: "bg-amber-500/20 text-amber-300",
    error: "bg-red-500/20 text-red-300",
  }[tone];
  return (
    <span className={`rounded px-1.5 py-0.5 text-xs ${className}`}>
      {children}
    </span>
  );
}

function MigrationCard({
  migrations,
  ok,
}: {
  migrations: AdminMigrationStatus | null;
  ok: boolean;
}) {
  if (!migrations) {
    return (
      <p className="rounded-md bg-red-900/40 px-3 py-2 text-sm text-red-200">
        マイグレーションの状態を読めませんでした (DB に繋がっていない可能性)
      </p>
    );
  }
  return (
    <div className="rounded-lg border border-white/10 bg-white/5 p-4">
      <div className="flex flex-wrap items-center gap-3">
        <h3 className="text-sm font-medium">マイグレーション</h3>
        {ok ? <Badge tone="ok">最新</Badge> : <Badge tone="warn">要確認</Badge>}
        <span className="text-xs text-neutral-400">
          適用済み {migrations.applied_count} 件
        </span>
      </div>
      {migrations.latest && (
        <p className="mt-2 text-xs text-neutral-400">
          最新: <span className="font-mono">{migrations.latest.version}</span>{" "}
          {migrations.latest.description} (
          {formatDateTime(migrations.latest.installed_on)}、
          {migrations.latest.execution_time_ms} ms)
        </p>
      )}
      <ul className="mt-2 flex flex-col gap-1 text-xs">
        {migrations.table_missing && (
          <li className="text-red-300">
            _sqlx_migrations テーブルがありません
            (一度もマイグレーションしていない DB)
          </li>
        )}
        {migrations.pending.length > 0 && (
          <li className="text-amber-300">
            未適用:{" "}
            {migrations.pending
              .map((m) => `${m.version} ${m.description}`)
              .join(" / ")}
          </li>
        )}
        {migrations.failed.length > 0 && (
          <li className="text-red-300">
            適用に失敗したまま残っている版: {migrations.failed.join(", ")}
          </li>
        )}
        {migrations.checksum_mismatch.length > 0 && (
          <li className="text-red-300">
            チェックサム不一致: {migrations.checksum_mismatch.join(", ")}{" "}
            (適用済みのマイグレーションファイルが書き換えられている)
          </li>
        )}
        {migrations.unknown.length > 0 && (
          <li className="text-amber-300">
            この api に無い版が適用済み: {migrations.unknown.join(", ")}{" "}
            (新しい版から切り戻した可能性)
          </li>
        )}
      </ul>
    </div>
  );
}

function GuildActivitySection({ stats }: { stats: AdminStats }) {
  return (
    <Section
      title="直近のギルド"
      description={
        <>
          guilds テーブルには参加・退出の日時が無いため、登録順 (id)
          と残っている予定の作成日時から推測している。Discord 側との突き合わせは
          <Link href={ROUTES.adminGuilds} className="ml-1 underline">
            ギルド一覧
          </Link>
          の差分検出から
        </>
      }
    >
      <div className="grid gap-4 lg:grid-cols-2">
        <div className="rounded-lg border border-white/10">
          <h3 className="border-b border-white/10 px-3 py-2 text-sm font-medium">
            最近登録されたギルド
          </h3>
          {stats.recent_guilds.length === 0 ? (
            <p className="px-3 py-4 text-sm text-neutral-500">まだありません</p>
          ) : (
            <ul className="divide-y divide-white/5">
              {stats.recent_guilds.map((guild) => (
                <li key={guild.guild_id}>
                  <Link
                    href={`${ROUTES.adminGuilds}/${guild.guild_id}`}
                    className="flex items-center gap-2 px-3 py-2 text-sm hover:bg-white/5"
                  >
                    {guild.avatar_url ? (
                      // biome-ignore lint/performance/noImgElement: Discord CDN のアイコンは最適化不要
                      <img
                        src={guild.avatar_url}
                        alt=""
                        className="h-5 w-5 rounded-full"
                      />
                    ) : (
                      <span
                        aria-hidden="true"
                        className="inline-flex h-5 w-5 items-center justify-center rounded-full bg-white/10 text-[10px]"
                      >
                        {guild.name.slice(0, 1)}
                      </span>
                    )}
                    <span className="truncate">{guild.name}</span>
                    <span className="ml-auto font-mono text-xs text-neutral-500">
                      {guild.guild_id}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="rounded-lg border border-white/10">
          <h3 className="border-b border-white/10 px-3 py-2 text-sm font-medium">
            退出済みで残っているデータ
          </h3>
          {stats.left_guilds.length === 0 ? (
            <p className="px-3 py-4 text-sm text-neutral-500">
              残っているデータはありません
            </p>
          ) : (
            <ul className="divide-y divide-white/5">
              {stats.left_guilds.map((guild) => (
                <li key={guild.guild_id}>
                  <Link
                    href={`${ROUTES.adminGuilds}/${guild.guild_id}`}
                    className="flex flex-wrap items-center gap-x-3 px-3 py-2 text-sm hover:bg-white/5"
                  >
                    <span className="font-mono text-xs">{guild.guild_id}</span>
                    <span className="text-neutral-400">
                      予定 {guild.event_count.toLocaleString()} 件
                    </span>
                    {guild.last_event_created_at && (
                      <span className="ml-auto text-xs text-neutral-500">
                        最終作成 {formatNaiveJst(guild.last_event_created_at)}
                      </span>
                    )}
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </Section>
  );
}
