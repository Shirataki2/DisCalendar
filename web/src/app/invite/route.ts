import { redirect } from "next/navigation";
import { botInviteUrl } from "@/lib/discord";

// LP の「BOT を導入する」の飛び先。招待 URL は実行時の環境変数 (DISCORD_BOT_INVITE_URL / DISCORD_CLIENT_ID) で決まり、
// ビルド時には分からないので、静的な LP には埋め込まずここでリダイレクトする。
// GET の Route Handler は既定で動的 (リクエストごとに評価される)
export function GET(): never {
  redirect(botInviteUrl());
}
