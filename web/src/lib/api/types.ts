// Rust API (api/) の入出力の型。api/src/models と api/src/routes の定義に対応する。
// 日時はすべてタイムゾーンなしの JST 文字列 ("2026-08-22T10:00:00")

export type NotificationUnit = "minutes" | "hours" | "days" | "weeks";

/** 予定開始の「num unit 前」に通知する */
export interface Notification {
  num: number;
  unit: NotificationUnit;
}

export interface ApiEvent {
  id: number;
  guild_id: string;
  name: string;
  description: string | null;
  notifications: Notification[];
  /** #RRGGBB */
  color: string;
  is_all_day: boolean;
  start_at: string;
  end_at: string;
  created_at: string;
  created_by: string | null;
  updated_by: string | null;
  updated_at: string | null;
  /** 連携している Discord スケジュールイベントの ID。未連携なら null */
  discord_scheduled_event_id: string | null;
}

/** 予定の作成・更新リクエスト (更新は全フィールド置き換え) */
export interface ApiEventInput {
  name: string;
  description?: string | null;
  notifications: Notification[];
  color: string;
  is_all_day: boolean;
  start_at: string;
  end_at: string;
  /**
   * Discord のスケジュールイベントとしても作成・同期する (#94)。
   * api 側は省略も受け付ける (更新では現在の連携状態を保持) が、web は常に明示して送る
   */
  discord_scheduled_event: boolean;
}

/** 横断カレンダー (#98) で一度に問い合わせられるサーバー数 (api の `events::JOINED_MAX_IDS`。超えると 400) */
export const JOINED_EVENTS_MAX_GUILDS = 200;

export interface Guild {
  guild_id: string;
  name: string;
  avatar_url: string | null;
  locale: string;
}

export interface GuildConfig {
  guild_id: string;
  /** true なら予定の作成・更新・削除にサーバー管理権限が必要 */
  restricted: boolean;
}

/**
 * 発行済みの iCal フィード (#95。api の `feed_tokens::FeedToken`)。
 * URL は web が自分のオリジンと token から組み立てる (`lib/feed-url.ts`)。
 * `GET /guilds/{guild_id}/feed` は未発行なら null を返す
 */
export interface GuildFeed {
  guild_id: string;
  /** `/feeds/<token>.ics` のパスに入る 64 文字の 16 進。知っている人は誰でも予定を読める */
  token: string;
  /** 発行日時 (JST) */
  created_at: string;
  /** 発行した Discord ユーザー ID */
  created_by: string;
}

/** GET /admin/me。ADMIN_DISCORD_USER_IDS に含まれるユーザーだけ 200 (それ以外は 403) */
export interface AdminMe {
  /** Better Auth の user.id */
  user_id: string;
  name: string;
  /** 連携済み Discord アカウントのユーザー ID (Snowflake、文字列) */
  discord_user_id: string;
}

/**
 * GET /admin/guilds の 1 行 (guilds + guild_config + event_settings + 予定数)。
 * Bot が退出すると guilds の行は消えるが予定や設定は残るので、そういうギルドも含まれる
 * (name などが null、registered が false)
 */
export interface AdminGuild {
  guild_id: string;
  /** guilds の名前。退出済みで行が消えていれば null */
  name: string | null;
  avatar_url: string | null;
  locale: string | null;
  /** guilds に行があるか (Bot が参加中として登録しているか) */
  registered: boolean;
  /** guild_config.restricted (行が無ければ false) */
  restricted: boolean;
  /** /init で設定した通知先チャンネル ID。未設定なら null */
  channel_id: string | null;
  /** 予定の総数 */
  event_count: number;
}

/** GET /admin/guilds の page の上限 (api の `admin_guilds::MAX_PAGE` と同じ。超えると 400) */
export const ADMIN_GUILDS_MAX_PAGE = 1_000_000;

export interface AdminGuildPage {
  items: AdminGuild[];
  /** 検索条件に一致する総件数 */
  total: number;
  /** 1 始まり */
  page: number;
  page_size: number;
}

/** GET /admin/guilds/{guild_id}。Bot の参加状況は Discord API に聞けなかったとき null */
export interface AdminGuildDetail extends AdminGuild {
  bot_joined: boolean | null;
}

/** POST /admin/sql の 1 カラム */
export interface SqlColumn {
  name: string;
  /** Postgres の型名 (INT4 / TEXT / TIMESTAMPTZ など) */
  type: string;
}

/** POST /admin/sql の結果。値は Postgres のテキスト表現 (psql と同じ)、NULL は null */
export interface SqlResult {
  columns: SqlColumn[];
  rows: (string | null)[][];
  row_count: number;
  /** ADMIN_SQL_MAX_ROWS 行か合計サイズ (4 MiB) を超えたので打ち切った */
  truncated: boolean;
  duration_ms: number;
}

/** SQL コンソールが 1 回で返す最大行数 (api の `admin_sql::MAX_ROWS`) */
export const ADMIN_SQL_MAX_ROWS = 500;
/** SQL の長さの上限 (文字数。api の `admin_sql::MAX_SQL_CHARS`、超えると 400) */
export const ADMIN_SQL_MAX_CHARS = 10_000;
/** 1 文の実行時間の上限 (秒。api の `admin_sql::STATEMENT_TIMEOUT`) */
export const ADMIN_SQL_TIMEOUT_SECONDS = 10;
/** 1 セルの文字数の上限 (api の `admin_sql::MAX_CELL_CHARS`。超えた分は切り詰めて末尾に印が付く) */
export const ADMIN_SQL_MAX_CELL_CHARS = 4_000;

/** GET /admin/sql/history の 1 件 (監査ログの sql.select から組み立てたもの) */
export interface SqlHistoryEntry {
  id: number;
  actor_discord_user_id: string;
  /** 実行した SQL。文字列リテラルは '…' に、コメントは除いて保存されている (貼り付けた秘密値を残さないため) */
  sql: string;
  row_count: number | null;
  truncated: boolean | null;
  duration_ms: number | null;
  /** 失敗・拒否時のメッセージ */
  error: string | null;
  /** ISO 8601 (UTC) */
  created_at: string;
}

// 稼働状況・ユーザー / セッション・監査ログ (#37。api/src/routes/admin_status.rs, admin_users.rs, admin_audit.rs)

/** GET /admin/stats の件数 */
export interface AdminCounts {
  /** guilds テーブルの行数 (Bot が参加中として登録しているギルド) */
  guilds: number;
  /** 予定・設定も含めて DB に痕跡があるギルドの数 (退出済みを含む) */
  known_guilds: number;
  /** known_guilds - guilds (退出済みでデータだけ残っているギルド) */
  left_guilds: number;
  events: number;
  /** まだ終わっていない予定 */
  upcoming_events: number;
  users: number;
  active_sessions: number;
  sessions: number;
}

/** 直近に guilds へ登録されたギルド */
export interface AdminRecentGuild {
  guild_id: string;
  name: string;
  avatar_url: string | null;
}

/** guilds に行が無いのにデータが残っているギルド (退出の痕跡) */
export interface AdminLeftGuild {
  guild_id: string;
  event_count: number;
  /** 残っている予定のうち最後に作られたもの (JST)。予定が無ければ null */
  last_event_created_at: string | null;
}

/** GET /admin/stats */
export interface AdminStats {
  counts: AdminCounts;
  /** 今日 (JST) 発火する通知の数 */
  notifications_today: number;
  /** 集計の基準にした今日の 0 時 (JST) */
  day_start: string;
  recent_guilds: AdminRecentGuild[];
  left_guilds: AdminLeftGuild[];
}

/** api の実行ファイルに埋め込まれたビルド情報 (未指定なら null) */
export interface AdminBuildInfo {
  version: string;
  git_sha: string | null;
  image_tag: string | null;
  debug: boolean;
}

/** _sqlx_migrations の 1 行 */
export interface AdminAppliedMigration {
  version: number;
  description: string;
  /** ISO 8601 (UTC) */
  installed_on: string;
  success: boolean;
  execution_time_ms: number;
}

export interface AdminPendingMigration {
  version: number;
  description: string;
}

/** マイグレーションの適用状況 */
export interface AdminMigrationStatus {
  applied_count: number;
  latest: AdminAppliedMigration | null;
  /** まだ適用されていない版 (通常は空) */
  pending: AdminPendingMigration[];
  /** success = false のまま残っている版 */
  failed: number[];
  /** 適用済みファイルを書き換えた版 (AGENTS.md の P0 に反する) */
  checksum_mismatch: number[];
  /** DB にはあるが今の api には無い版 */
  unknown: number[];
  table_missing: boolean;
}

export interface AdminDatabaseStatus {
  reachable: boolean;
  latency_ms: number | null;
  server_version: string | null;
  /** 繋がらなかったときの理由 */
  error: string | null;
  pool_connections: number;
  pool_idle: number;
}

/** GET /admin/status */
export interface AdminStatus {
  build: AdminBuildInfo;
  /** ISO 8601 (UTC) */
  started_at: string;
  uptime_seconds: number;
  database: AdminDatabaseStatus;
  /** DB に繋がらなければ null */
  migrations: AdminMigrationStatus | null;
  migrations_ok: boolean;
}

/** GET /admin/guilds/sync-check の差分の 1 件 */
export interface AdminSyncGuild {
  guild_id: string;
  name: string | null;
}

export interface AdminNameMismatch {
  guild_id: string;
  db_name: string;
  discord_name: string;
}

/** GET /admin/guilds/sync-check。一覧は種類ごとに ADMIN_SYNC_LIST_LIMIT 件まで (件数は *_count) */
export interface AdminGuildSyncCheck {
  discord_count: number;
  db_count: number;
  only_in_db: AdminSyncGuild[];
  only_in_db_count: number;
  only_in_discord: AdminSyncGuild[];
  only_in_discord_count: number;
  name_mismatch: AdminNameMismatch[];
  name_mismatch_count: number;
}

/** 差分検出が一覧で返す件数の上限 (api の `admin_status::SYNC_LIST_LIMIT`) */
export const ADMIN_SYNC_LIST_LIMIT = 200;

/** GET /admin/users の 1 行。トークン類は含まない */
export interface AdminUserSummary {
  /** Better Auth の user.id */
  id: string;
  name: string;
  email: string;
  image: string | null;
  /** ISO 8601 (UTC) */
  created_at: string;
  /** 連携済み Discord アカウントのユーザー ID (Snowflake、文字列) */
  discord_user_id: string | null;
  active_sessions: number;
  sessions: number;
  last_session_at: string | null;
}

export interface AdminUserPage {
  items: AdminUserSummary[];
  total: number;
  /** 1 始まり */
  page: number;
  page_size: number;
}

/** GET /admin/users の page の上限 (api の `admin_users::MAX_PAGE`) */
export const ADMIN_USERS_MAX_PAGE = 1_000_000;
/** 1 ユーザーについて返すセッションの上限 (api の `admin_users::SESSION_LIMIT`) */
export const ADMIN_USER_SESSION_LIMIT = 100;

/** GET /admin/users/{id}/sessions の 1 件。**セッショントークンは含まれない** */
export interface AdminSession {
  /** session.id (認証に使う token とは別の値) */
  id: string;
  created_at: string;
  updated_at: string;
  expires_at: string;
  ip_address: string | null;
  user_agent: string | null;
  expired: boolean;
}

/** 監査ログ 1 件 (admin_audit_logs) */
export interface AdminAuditLog {
  id: number;
  actor_user_id: string;
  actor_discord_user_id: string;
  /** event.update / guild_config.update / sql.select / user.revoke_sessions など */
  action: string;
  target_type: string | null;
  target_id: string | null;
  before: unknown | null;
  after: unknown | null;
  detail: unknown | null;
  /** ISO 8601 (UTC) */
  created_at: string;
}

export interface AdminAuditLogPage {
  items: AdminAuditLog[];
  total: number;
  page: number;
  page_size: number;
  /** 記録されている操作の種類 (絞り込みの選択肢) */
  actions: string[];
}

/** GET /admin/audit-logs の page の上限 (api の `admin_audit::MAX_PAGE`) */
export const ADMIN_AUDIT_LOGS_MAX_PAGE = 1_000_000;

/** 全予定削除で監査ログに残す予定のスナップショット上限 (api の `admin_ops::DELETE_SNAPSHOT_LIMIT`)。超えた分は件数だけ残る */
export const ADMIN_DELETE_SNAPSHOT_LIMIT = 200;

/** POST /admin/ops/* の結果 */
export interface OpsResult {
  deleted: number;
}

export interface MyPermissions {
  user_id: string;
  permissions: string;
  administrator: boolean;
  manage_guild: boolean;
  manage_messages: boolean;
  manage_roles: boolean;
  /** 上記 4 つのいずれか */
  can_manage_server: boolean;
  /**
   * このユーザー自身が Discord の「イベントの作成」権限を持つか (#94)。
   * 連携は Bot が代行するので、これが false のユーザーは予定を新たに連携させられない
   */
  create_events: boolean;
  /**
   * Bot 自身が「イベントの作成」権限を持つか (#94)。
   * 予定ダイアログの「Discord のイベントとしても作成する」の出し分けに使う
   */
  bot_create_events: boolean;
}

// 分析情報 (#79。api/src/routes/admin_analytics.rs, api/src/models/admin_analytics.rs)
// 指標の定義と精度の限界は api 側のモジュールコメントに書いてある

/** ある期間の件数と、その直前の同じ長さの期間との比較 */
export interface AdminTrend {
  current: number;
  previous: number;
  /** current - previous */
  delta: number;
  /** 増減率 (%)。previous が 0 なら計算できないので null */
  change_percent: number | null;
}

/** アクティブユーザー (セッションの生存期間がその期間に重なる利用者の数。推定) */
export interface AdminActiveUsers {
  /** 直近 1 日 (DAU) */
  daily: AdminTrend;
  /** 直近 7 日 (WAU) */
  weekly: AdminTrend;
  /** 直近 30 日 (MAU) */
  monthly: AdminTrend;
}

/**
 * 実測のアクティブユーザー (#81)。利用の記録 (1 人 1 日 1 行) から数える。
 * 推定 (AdminActiveUsers) が「今」からの 24 時間刻みなのに対して、こちらは JST の日付で区切る
 */
export interface AdminMeasuredActiveUsers {
  /** 記録が始まった日 (`2026-08-28`)。まだ 1 行も無ければ null */
  since: string | null;
  /** 今日 (まだ途中)。前は昨日 */
  daily: AdminTrend;
  /** 今日までの 7 日間。前はその前の 7 日間 */
  weekly: AdminTrend;
  /** 今日までの 30 日間。前はその前の 30 日間 */
  monthly: AdminTrend;
}

/** 日別の 1 点。date は JST の日付 (`2026-08-25`) */
export interface AdminDailyPoint {
  date: string;
  events: number;
  new_users: number;
  /** その日に作られたセッション (ログイン) の数 */
  logins: number;
  /** その日に使った利用者 (実測、#81)。記録開始前の日は 0 */
  measured_active_users: number;
}

/** 月別の 1 点。month は JST の月初 (`2026-08-01`) */
export interface AdminMonthlyPoint {
  month: string;
  /** その月にセッションの生存期間が重なった利用者 (その月の MAU) */
  active_users: number;
  new_users: number;
  logins: number;
  events: number;
}

/** 予定の作成数 */
export interface AdminEventCreation {
  last_day: AdminTrend;
  last_week: AdminTrend;
  last_month: AdminTrend;
  /** 今 DB にある予定の総数 (削除されたものは含まれない) */
  total: number;
}

/** 予定の内訳と設定の行き渡り具合 */
export interface AdminAnalyticsBreakdown {
  all_day_events: number;
  /**
   * 開始時刻の通知に追加で通知が飛ぶ予定の数。
   * 解釈できない旧データの設定と、発火時刻が重なる設定 (Bot が 1 回にまとめる) は含めない
   */
  events_with_notifications: number;
  /** 予定 1 件あたりの追加の通知数の平均 */
  notifications_per_event: number;
  /**
   * 参加中のギルドのうち、通知先チャンネルに形式として正しい ID を設定しているものの数。
   * チャンネルの存在や Bot の送信権限までは確認していない
   */
  guilds_with_channel: number;
  restricted_guilds: number;
}

/** 予定の作成数が多いギルド */
export interface AdminTopGuild {
  guild_id: string;
  /** guilds にある名前。退出済みで行が無ければ null */
  name: string | null;
  avatar_url: string | null;
  event_count: number;
}

export interface AdminGuildActivity {
  /** 直近 N 日に予定が作られた参加中のギルドの数 (割合の分子。退出済みは含まない) */
  active_guilds: number;
  /** 直近 N 日に予定が作られたが guilds に行が無いギルドの数 (退出済みの残骸) */
  active_left_guilds: number;
  /** guilds テーブルの行数 (割合の分母) */
  joined_guilds: number;
  /** 退出済みも含む */
  top_guilds: AdminTopGuild[];
}

/** GET /admin/analytics */
export interface AdminAnalytics {
  /** 集計の基準にした日 (JST、`2026-08-25`)。推移の右端で、その日はまだ途中 */
  today: string;
  daily_days: number;
  monthly_months: number;
  /** 「アクティブ」「直近」の基準にした日数 */
  recent_days: number;
  /** セッションからの推定 (#79) */
  active_users: AdminActiveUsers;
  /** 利用の記録から数えた実測 (#81) */
  measured_active_users: AdminMeasuredActiveUsers;
  event_creation: AdminEventCreation;
  breakdown: AdminAnalyticsBreakdown;
  guilds: AdminGuildActivity;
  /** 古い順 */
  daily: AdminDailyPoint[];
  /** 古い順 */
  monthly: AdminMonthlyPoint[];
}

export type ApiErrorKind =
  | "unauthorized"
  | "forbidden"
  /** Bot の権限不足 (#94)。利用者自身の権限不足と違い、Bot の再招待で直る */
  | "bot_permission"
  | "not_found"
  | "bad_request"
  | "conflict"
  | "rate_limited"
  | "unavailable"
  | "discord_error"
  | "database_error"
  | "internal_error";

export interface ApiErrorBody {
  error: ApiErrorKind;
  message: string;
}

/** 発行済みの予定共有リンク。 */
export interface ShareLink {
  token: string;
}
/** 公開する項目のみ。日時は JST。 */
export interface SharedEvent {
  guild_id: string;
  guild_name: string;
  guild_avatar_url: string | null;
  name: string;
  description: string | null;
  is_all_day: boolean;
  start_at: string;
  end_at: string;
}

/** ギルド内の表示情報。退出したメンバーは ID のみ。 */
export interface MemberProfile {
  user_id: string;
  display_name: string | null;
  avatar_url: string | null;
}
