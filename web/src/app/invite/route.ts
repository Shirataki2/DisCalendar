import { redirect } from "next/navigation";
import { after, type NextRequest } from "next/server";
import { botInviteUrl } from "@/lib/discord";
import { recordInviteClick } from "@/lib/invite-clicks";

// LP の「BOT を導入する」の飛び先。招待 URL は実行時の環境変数 (DISCORD_BOT_INVITE_URL / DISCORD_CLIENT_ID) で決まり、
// ビルド時には分からないので、静的な LP には埋め込まずここでリダイレクトする。
// GET の Route Handler は既定で動的 (リクエストごとに評価される)。
//
// `?from=<導入元>` (lib/site.ts の inviteHref、Bot 掲載サイトなど) を導入元ごとのクリック数として記録する (#374)。
// 記録はリダイレクトを返した後 (after) に行い、招待画面が開くのを待たせない。
// IP・cookie・利用者の ID は送らない
export function GET(request: NextRequest): never {
  const source = request.nextUrl.searchParams.get("from");
  after(() => recordInviteClick(source));
  redirect(botInviteUrl());
}
