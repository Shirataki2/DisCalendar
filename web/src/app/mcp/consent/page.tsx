import { headers } from "next/headers";
import { notFound, redirect } from "next/navigation";
import { auth } from "@/lib/auth";
import { getUserGuilds } from "@/lib/discord";
import { loadJoinedGuildIds } from "@/lib/joined-guilds";
import { MCP_SCOPES, mcpEnabled } from "@/lib/mcp/config";

export default async function McpConsentPage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  // 停止状態をビルド時の404として固定せず、実行時に判定する。
  const requestHeaders = await headers();
  if (!mcpEnabled()) notFound();
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
  const scopes = (params.get("scope") ?? "").split(" ");
  return (
    <main className="mx-auto max-w-2xl space-y-6 p-8">
      <h1 className="text-2xl font-bold">MCP 接続を許可</h1>
      <p className="break-all">
        接続するクライアント: {params.get("client_id")}
      </p>
      <p>
        {session.user.name}{" "}
        として許可します。選択したサーバー名や予定のタイトル・説明・日時が接続先に公開されます。DiscordのトークンやWebのログイン情報は渡しません。
      </p>
      <p>
        作成・変更・削除を許可すると、DisCalendarは操作ごとの確認を求めません。接続先の確認設定は変わりません。現在は接続内容の確認のみ利用できます。予定の操作にはまだ対応していません。
      </p>
      <form action="/mcp/consent/submit" method="post" className="space-y-6">
        <input type="hidden" name="oauth_query" value={params.toString()} />
        <fieldset className="space-y-2">
          <legend className="font-semibold">許可する操作</legend>
          {Object.entries(MCP_SCOPES)
            .filter(([scope]) => scopes.includes(scope))
            .map(([scope, label]) => (
              <label key={scope} className="flex gap-2">
                <input type="checkbox" name="scope" value={scope} />
                {label}
              </label>
            ))}
        </fieldset>
        <fieldset className="space-y-2">
          <legend className="font-semibold">許可するサーバー</legend>
          {guilds
            .filter((g) => joined.ids.has(g.id))
            .map((g) => (
              <label key={g.id} className="flex gap-2">
                <input type="checkbox" name="guild_id" value={g.id} />
                {g.name}
              </label>
            ))}
        </fieldset>
        <p>
          後から参加するサーバーは追加されません。接続管理からいつでも解除できます。
        </p>
        <div className="flex gap-4">
          <button
            type="submit"
            name="accept"
            value="true"
            className="rounded bg-indigo-600 px-5 py-2 text-white"
          >
            選択した内容を許可
          </button>
          <button
            type="submit"
            name="accept"
            value="false"
            className="rounded border px-5 py-2"
          >
            拒否
          </button>
        </div>
      </form>
    </main>
  );
}
