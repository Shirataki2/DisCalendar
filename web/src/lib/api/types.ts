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
}

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
}

export type ApiErrorKind =
  | "unauthorized"
  | "forbidden"
  | "not_found"
  | "bad_request"
  | "rate_limited"
  | "unavailable"
  | "discord_error"
  | "database_error"
  | "internal_error";

export interface ApiErrorBody {
  error: ApiErrorKind;
  message: string;
}
