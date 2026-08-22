import { apiFetch } from "./client";
import { createApi } from "./endpoints";

/** ブラウザ用の API クライアント (Next.js の rewrites 経由)。RSC からは "@/lib/api/server" を使う */
export const api = createApi(apiFetch);

export { ApiError, describeApiError } from "./client";
export type * from "./types";
