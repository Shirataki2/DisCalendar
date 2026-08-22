import { headers } from "next/headers";
import {
  type ApiFetcher,
  buildInit,
  handleResponse,
  type RequestOptions,
} from "./client";
import { createApi } from "./endpoints";

// next.config.ts の rewrites と同じ値。RSC からはプロキシを通さず API を直接呼ぶ
const API_URL = process.env.API_URL ?? "http://127.0.0.1:8080";

/**
 * Server Component / Route Handler から Rust API を呼ぶ。
 * ブラウザから受け取ったリクエストの cookie (Better Auth のセッション) をそのまま転送する
 */
const serverApiFetch: ApiFetcher = async <T>(
  path: string,
  options: RequestOptions = {},
): Promise<T> => {
  const cookie = (await headers()).get("cookie");
  const init = buildInit(options);
  const res = await fetch(`${API_URL}${path}`, {
    ...init,
    headers: { ...init.headers, ...(cookie ? { cookie } : {}) },
    cache: "no-store",
  });
  return handleResponse<T>(res);
};

export const serverApi = createApi(serverApiFetch);
