import { ActivityApp } from "@/components/activity/activity-app";
import { botInviteUrl } from "@/lib/discord";

// DISCORD_CLIENT_ID は実行時の環境変数なので、ビルド時に固めずリクエストごとに読む
export const dynamic = "force-dynamic";

/**
 * Discord の Activity (#350) の入口。Discord はアプリのルート (`/`) を `frame_id` などのクエリ付きで開くので、
 * proxy.ts がそれをこのページへ rewrite する。認証と描画はすべてクライアント側 (ActivityApp) で行う
 */
export default function ActivityPage() {
  return (
    <ActivityApp
      clientId={process.env.DISCORD_CLIENT_ID ?? ""}
      inviteUrl={botInviteUrl()}
    />
  );
}
