import { serverApi } from "@/lib/api/server";
import type { DiscordGuild } from "@/lib/discord";

// サーバー選択画面 (/dashboard) と横断カレンダー (/dashboard/all、#98) で共通の
// 「ユーザーの参加サーバーのうち Bot が入っているもの」の求め方。
// serverApi (RSC 用) に依存するので Server Component からだけ呼ぶ

export type JoinedGuildIds =
  | { ok: true; ids: Set<string> }
  | { ok: false; error: unknown };

/**
 * Discord から取ったユーザーの参加サーバーのうち、Bot が参加している (カレンダーが使える) ものの ID。
 * api に届かないなどの失敗はエラーを添えて返し、画面ごとに扱いを決める
 */
export async function loadJoinedGuildIds(
  guilds: DiscordGuild[],
): Promise<JoinedGuildIds> {
  try {
    const joined = await serverApi.guilds.joined(guilds.map((g) => g.id));
    return { ok: true, ids: new Set(joined.map((g) => g.guild_id)) };
  } catch (error) {
    return { ok: false, error };
  }
}
