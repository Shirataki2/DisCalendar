import type { ApiErrorBody, ApiErrorKind } from "./types";

/** ブラウザから見た API のプレフィックス。next.config.ts の rewrites で Rust API へプロキシされる */
export const API_PREFIX = "/local/api";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly kind: ApiErrorKind | "unknown",
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

export interface RequestOptions {
  method?: "GET" | "POST" | "PUT" | "DELETE";
  /** JSON として送る */
  body?: unknown;
  signal?: AbortSignal;
}

/** API を 1 回呼び出す関数。ブラウザ (apiFetch) と RSC (serverApiFetch) で実装が違う */
export type ApiFetcher = <T>(
  path: string,
  options?: RequestOptions,
) => Promise<T>;

/**
 * ブラウザから API を呼び出してレスポンス JSON を返す (204 は undefined)。
 * 2xx 以外は ApiError を投げる。同一オリジンなのでセッション cookie は自動で付く
 */
export const apiFetch: ApiFetcher = async <T>(
  path: string,
  options: RequestOptions = {},
): Promise<T> => {
  const res = await fetch(`${API_PREFIX}${path}`, buildInit(options));
  return handleResponse<T>(res);
};

export function buildInit(options: RequestOptions): RequestInit {
  const hasBody = options.body !== undefined;
  return {
    method: options.method ?? "GET",
    headers: hasBody ? { "content-type": "application/json" } : undefined,
    body: hasBody ? JSON.stringify(options.body) : undefined,
    signal: options.signal,
  };
}

export async function handleResponse<T>(res: Response): Promise<T> {
  if (res.status === 204) {
    return undefined as T;
  }
  if (!res.ok) {
    throw await toApiError(res);
  }
  return (await res.json()) as T;
}

async function toApiError(res: Response): Promise<ApiError> {
  let body: Partial<ApiErrorBody> | null = null;
  try {
    body = (await res.json()) as Partial<ApiErrorBody>;
  } catch {
    // JSON でないレスポンス (プロキシ先に届かなかった場合など)
  }
  return new ApiError(
    res.status,
    body?.error ?? "unknown",
    body?.message ?? `API request failed with status ${res.status}`,
  );
}

/** ユーザーに見せる日本語のエラーメッセージ */
export function describeApiError(error: unknown): string {
  if (!(error instanceof ApiError)) {
    return error instanceof Error && error.name === "AbortError"
      ? "通信が中断されました"
      : "通信に失敗しました。ネットワークを確認してください";
  }
  switch (error.kind) {
    case "unauthorized":
      return "ログインの有効期限が切れました。再度ログインしてください";
    case "forbidden":
      return "この操作を行う権限がありません";
    case "bot_permission":
      return "Bot に「イベントの作成」権限がないため Discord に反映できませんでした。Bot を招待し直すと利用できます (使い方の「サーバーに導入する」)";
    case "not_found":
      return "対象が見つかりません (他のユーザーが削除した可能性があります)";
    case "bad_request":
      return `入力内容が正しくありません (${error.message})`;
    case "conflict":
      return "他の更新と同時に行われたため保存できませんでした。もう一度お試しください";
    case "rate_limited":
      return "Discord API の制限中です。しばらく待ってから再度お試しください";
    case "unavailable":
      return `この機能は現在使えません (${error.message})`;
    case "discord_error":
      return "Discord との通信に失敗しました。時間をおいて再度お試しください";
    default:
      return `サーバーでエラーが発生しました (${error.status})`;
  }
}
