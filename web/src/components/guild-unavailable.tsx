import type { ReactNode } from "react";
import { Message } from "@/components/language-provider";
import { ApiError } from "@/lib/api/client";

/**
 * サーバーのカレンダーを開けないときの案内 (メンバーでない / Bot 未参加 / Discord との通信失敗)。
 * 通常の Web (dashboard/[id]) と Discord の Activity (#350) で共用し、下に置く操作だけを `action` で差し替える
 */
export function GuildUnavailable({
  error,
  action,
}: {
  error: unknown;
  action?: ReactNode;
}) {
  const discordTrouble =
    error instanceof ApiError &&
    (error.kind === "discord_error" || error.kind === "rate_limited");
  return (
    <main className="flex flex-1 flex-col items-center justify-center gap-4 p-8 text-center">
      <h1 className="text-xl font-bold">
        <Message message={"サーバーデータの取得に失敗しました"} />
      </h1>
      {discordTrouble ? (
        <p className="text-sm text-muted-foreground">
          <Message
            message={
              "Discord との通信に失敗しました。時間をおいて再度お試しください。"
            }
          />
        </p>
      ) : (
        <div className="text-sm text-muted-foreground">
          <p className="mb-2">
            <Message message={"以下の事項をご確認ください"} />
          </p>
          <p>
            <Message message={"･ BOTがサーバーに導入されているか"} />
          </p>
          <p>
            <Message
              message={"･ あなた自身がBOTを導入したサーバーに参加しているか"}
            />
          </p>
        </div>
      )}
      {action}
    </main>
  );
}
