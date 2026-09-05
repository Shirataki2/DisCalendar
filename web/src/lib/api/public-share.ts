import "server-only";
import { cache } from "react";
import type { SharedEvent } from "./types";

/** Cookie を転送しない公開取得。失効・編集を反映するため永続キャッシュに置かない。 */
export const getSharedEvent = cache(
  async (token: string): Promise<SharedEvent | null> => {
    if (!/^[0-9a-f]{64}$/.test(token)) return null;
    const response = await fetch(
      `${process.env.API_URL ?? "http://127.0.0.1:8080"}/share/${token}`,
      { cache: "no-store" },
    );
    if (response.status === 404) return null;
    if (!response.ok) throw new Error("共有予定を取得できませんでした");
    return response.json();
  },
);
