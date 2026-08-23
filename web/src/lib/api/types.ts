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
  | "discord_error"
  | "database_error"
  | "internal_error";

export interface ApiErrorBody {
  error: ApiErrorKind;
  message: string;
}
