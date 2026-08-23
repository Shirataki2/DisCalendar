import { cache } from "react";
import { ApiError } from "@/lib/api";
import { serverApi } from "@/lib/api/server";
import type { AdminMe } from "@/lib/api/types";

/**
 * ログイン中のユーザーが管理コンソールの管理者 (api の `ADMIN_DISCORD_USER_IDS`) なら
 * その情報を、管理者でなければ null を返す。
 *
 * 判定は api (`GET /admin/me`、`AdminUser` extractor) に一本化し、web 側に ID の一覧を持たない。
 * これにより web の表示制御と api の認可がずれることがない。
 * 未認証 (401) はここでは扱わず呼び出し側 (`requireSession`) に任せる。
 * api に届かないなどの失敗はそのまま投げるので、リンクの表示のような「無くても困らない」用途では
 * 呼び出し側で握りつぶす。同一リクエスト内では React の `cache` で 1 回しか呼ばない
 */
export const getAdminMe = cache(async (): Promise<AdminMe | null> => {
  try {
    return await serverApi.admin.me();
  } catch (error) {
    if (error instanceof ApiError && error.status === 403) {
      return null;
    }
    throw error;
  }
});
