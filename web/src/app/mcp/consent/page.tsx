import { headers } from "next/headers";
import { notFound, redirect } from "next/navigation";
import { McpConsentForm } from "@/components/mcp-consent-form";
import { auth } from "@/lib/auth";
import { getUserGuilds } from "@/lib/discord";
import { loadJoinedGuildIds } from "@/lib/joined-guilds";
import { mcpConnectionsEnabled } from "@/lib/mcp/config";

export default async function McpConsentPage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  // 停止状態をビルド時の404として固定せず、実行時に判定する。
  const requestHeaders = await headers();
  if (!mcpConnectionsEnabled()) notFound();
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(await searchParams)) {
    for (const item of Array.isArray(value)
      ? value
      : value === undefined
        ? []
        : [value])
      params.append(key, item);
  }
  if (params.toString().length > 16_384) notFound();
  const session = await auth.api.getSession({ headers: requestHeaders });
  if (!session) redirect(`/mcp/login?${params}`);
  const guilds = await getUserGuilds();
  const joined = await loadJoinedGuildIds(guilds);
  if (!joined.ok) throw new Error("サーバー一覧を取得できませんでした。");
  return (
    <main className="mx-auto w-full max-w-3xl space-y-6 px-4 py-8 sm:px-8">
      <h1 className="text-2xl font-bold">MCP 接続を許可</h1>
      <p className="break-all">
        接続するクライアント: {params.get("client_id")}
      </p>
      <p className="text-sm leading-relaxed text-muted-foreground">
        {session.user.name}{" "}
        として許可します。選択したサーバー名や予定のタイトル・説明・日時が接続先に公開されます。DiscordのトークンやWebのログイン情報は渡しません。
      </p>
      <p className="text-sm leading-relaxed text-muted-foreground">
        作成・変更・削除を許可すると、DisCalendarは操作ごとの確認を求めません。接続先の確認設定は変わりません。許可した範囲で予定の閲覧・作成・変更・削除ができます。
      </p>
      <McpConsentForm
        key={params.toString()}
        query={params.toString()}
        guilds={guilds
          .filter((guild) => joined.ids.has(guild.id))
          .map(({ id, name }) => ({ id, name }))}
      />
    </main>
  );
}
