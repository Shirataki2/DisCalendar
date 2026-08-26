import { createServer, type Server } from "node:http";
import { DISCORD_MOCK_URL } from "./env";
import {
  E2E_ALL_GUILDS,
  E2E_BOT_TOKEN,
  E2E_USER,
  type E2EGuild,
  OTHER_OWNER_ID,
} from "./fixtures";

// Discord API (https://discord.com/api/v10) のモック。web と api の DISCORD_API_BASE_URL をここに向ける。
// 実装しているのは DisCalendar が呼ぶエンドポイントだけ:
// - GET /users/@me/guilds            web (lib/discord.ts、ユーザーのトークン) と api の管理コンソール (Bot トークン)
// - GET /guilds/{id}                 api (ギルド情報とロール一覧、権限計算)
// - GET /guilds/{id}/members/{uid}   api (メンバー確認と所持ロール)
// それ以外と未知のギルドは Discord と同じく 404 の JSON を返す。
// テストから Bot の参加状況を変える PUT / DELETE /_test/guilds/{id} (setBotJoined) だけは Discord に無い追加

/** Bot が参加しているギルド。テスト中に setBotJoined で増減する */
const joinedGuilds = new Map<string, E2EGuild>(
  E2E_ALL_GUILDS.filter((g) => g.botJoined).map((g) => [g.id, g]),
);

/**
 * テスト中に Bot がギルドに参加した / 退出した状態にする (招待の再現)。
 * モックは globalSetup のプロセスで動いていてテストからは直接触れないので、HTTP で伝える
 */
export async function setBotJoined(
  guildId: string,
  botJoined: boolean,
): Promise<void> {
  const res = await fetch(`${DISCORD_MOCK_URL}/_test/guilds/${guildId}`, {
    method: botJoined ? "PUT" : "DELETE",
  });
  if (!res.ok) {
    throw new Error(
      `Discord モックの参加状況を変更できませんでした (${guildId}): ${res.status}`,
    );
  }
}

/** `GET /users/@me/guilds` の 1 件 (ユーザーから見たギルド) */
function userGuild(guild: E2EGuild) {
  return {
    id: guild.id,
    name: guild.name,
    icon: null,
    owner: guild.owner,
    permissions: guild.permissions,
    features: [],
  };
}

/** `GET /guilds/{id}` (Bot から見たギルド)。@everyone ロールは id == guild_id */
function botGuild(guild: E2EGuild) {
  return {
    id: guild.id,
    name: guild.name,
    icon: null,
    owner_id: guild.owner ? E2E_USER.discordId : OTHER_OWNER_ID,
    roles: [{ id: guild.id, name: "@everyone", permissions: "1024" }],
  };
}

export function startDiscordMock(port: number): Promise<Server> {
  const server = createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://discord.local");
    const auth = req.headers.authorization ?? "";
    const json = (status: number, body: unknown) => {
      res.writeHead(status, { "content-type": "application/json" });
      res.end(JSON.stringify(body));
    };
    const notFound = (message: string, code: number) =>
      json(404, { message, code });

    // テスト専用 (Discord には無い): Bot の参加状況を変える
    const testMatch = /^\/_test\/guilds\/(\d+)$/.exec(url.pathname);
    if (testMatch && (req.method === "PUT" || req.method === "DELETE")) {
      const guild = E2E_ALL_GUILDS.find((g) => g.id === testMatch[1]);
      if (!guild) {
        return notFound("Unknown Guild", 10004);
      }
      if (req.method === "PUT") {
        joinedGuilds.set(guild.id, guild);
      } else {
        joinedGuilds.delete(guild.id);
      }
      return json(200, { joined: joinedGuilds.has(guild.id) });
    }

    if (req.method !== "GET") {
      return json(405, { message: "Method Not Allowed", code: 0 });
    }

    if (url.pathname === "/users/@me/guilds") {
      if (auth === `Bearer ${E2E_USER.accessToken}`) {
        return json(200, E2E_ALL_GUILDS.map(userGuild));
      }
      if (auth === `Bot ${E2E_BOT_TOKEN}`) {
        return json(
          200,
          [...joinedGuilds.values()].map((g) => ({
            id: g.id,
            name: g.name,
            icon: null,
          })),
        );
      }
      return json(401, { message: "401: Unauthorized", code: 0 });
    }

    const guildMatch = /^\/guilds\/(\d+)(?:\/members\/(\d+))?$/.exec(
      url.pathname,
    );
    if (guildMatch) {
      if (auth !== `Bot ${E2E_BOT_TOKEN}`) {
        return json(401, { message: "401: Unauthorized", code: 0 });
      }
      const [, guildId, userId] = guildMatch;
      const guild = joinedGuilds.get(guildId);
      if (!guild) {
        return notFound("Unknown Guild", 10004);
      }
      if (userId === undefined) {
        return json(200, botGuild(guild));
      }
      if (userId !== E2E_USER.discordId) {
        return notFound("Unknown Member", 10007);
      }
      return json(200, {
        user: { id: E2E_USER.discordId, username: E2E_USER.name },
        roles: [],
      });
    }

    return notFound("404: Not Found", 0);
  });

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => resolve(server));
  });
}
