"use server";

import { revalidatePath } from "next/cache";
import { ROUTES } from "@/lib/site";

/**
 * 管理コンソールで restricted や予定 (件数) を変えた後に呼ぶ。
 * ギルド一覧 (`/admin/guilds`) は RSC が描画しているので、TanStack Query のキャッシュを更新しても
 * ブラウザの戻る操作で Router Cache に残った一覧が古い値のまま出る。Server Action からの
 * `revalidatePath` はクライアントの Router Cache も捨てるので、`/admin` 配下をまとめて無効化する
 * (データの正は api にあり、この関数は再取得を促すだけなので誰が呼んでも害はない)
 */
export async function revalidateAdminPages() {
  revalidatePath(ROUTES.admin, "layout");
}
