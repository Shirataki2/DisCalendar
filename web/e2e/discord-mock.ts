import { createServer, type Server } from "node:http";
import { DISCORD_MOCK_URL } from "./env";
import {
  E2E_ALL_GUILDS,
  E2E_BOT_ROLE_ID,
  E2E_BOT_TOKEN,
  E2E_BOT_USER_ID,
  E2E_USER,
  E2E_USER_ROLE_ID,
  type E2EGuild,
  OTHER_OWNER_ID,
} from "./fixtures";

// Discord API (https://discord.com/api/v10) のモック。web と api の DISCORD_API_BASE_URL をここに向ける。
// 実装しているのは DisCalendar が呼ぶエンドポイントだけ:
// - GET /users/@me                              api (Bot 自身のユーザー ID、#94)
// - GET /users/@me/guilds                       web (lib/discord.ts、ユーザーのトークン) と api の管理コンソール (Bot トークン)
// - GET /guilds/{id}                            api (ギルド情報とロール一覧、権限計算)
// - GET /guilds/{id}/members/{uid}              api (メンバー確認と所持ロール。Bot 自身も含む)
// - POST/PATCH/DELETE /guilds/{id}/scheduled-events(/{sid})  api (スケジュールイベントの同期、#94)
// それ以外と未知のギルドは Discord と同じく 404 の JSON を返す。
// テスト用に Discord には無い経路を 2 つ足してある (モックは globalSetup のプロセスで動いていて
// テストからは直接触れないので、HTTP で伝える):
// - PUT / DELETE /_test/guilds/{id}              Bot の参加状況 (setBotJoined)
// - PUT / DELETE /_test/guilds/{id}/permissions  「イベントの作成」権限 (setGuildEventPermissions、#122)

/** Bot が参加しているギルド。テスト中に setBotJoined で増減する */
const joinedGuilds = new Map<string, E2EGuild>(
  E2E_ALL_GUILDS.filter((g) => g.botJoined).map((g) => [g.id, g]),
);

/** 「イベントの作成」権限の差し替え (#122)。fixtures の値は書き換えず、ここで上書きする */
type EventPermissions = Pick<E2EGuild, "botCreateEvents" | "userCreateEvents">;
const permissionOverrides = new Map<string, EventPermissions>();

/** モックが今返すべきギルド (差し替えを反映したもの) */
function currentGuild(guildId: string): E2EGuild | undefined {
  const guild = joinedGuilds.get(guildId);
  const override = guild && permissionOverrides.get(guildId);
  return override ? { ...guild, ...override } : guild;
}

/**
 * テスト中にギルドの「イベントの作成」権限を変える (Bot の招待し直しやロール付与の再現、#122)。
 * `null` で差し替えを取り消す。
 *
 * api は権限を数分キャッシュするので、これを呼んだだけでは api から見た権限は変わらない
 * (変えたあと `POST /guilds/{id}/@me/permissions/refresh` を通ると反映される)
 */
export async function setGuildEventPermissions(
  guildId: string,
  permissions: EventPermissions | null,
): Promise<void> {
  const query = new URLSearchParams(
    permissions
      ? {
          bot: String(permissions.botCreateEvents),
          user: String(permissions.userCreateEvents),
        }
      : {},
  );
  const res = await fetch(
    `${DISCORD_MOCK_URL}/_test/guilds/${guildId}/permissions?${query}`,
    { method: permissions ? "PUT" : "DELETE" },
  );
  if (!res.ok) {
    throw new Error(
      `Discord モックの権限を変更できませんでした (${guildId}): ${res.status}`,
    );
  }
}

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
    roles: [
      { id: guild.id, name: "@everyone", permissions: "1024" },
      // botCreateEvents / userCreateEvents のギルドでは「イベントの作成」(1<<44) のロールが付いている
      ...(guild.botCreateEvents
        ? [
            {
              id: E2E_BOT_ROLE_ID,
              name: "DisCalendar",
              permissions: "17592186044416",
            },
          ]
        : []),
      ...(guild.userCreateEvents
        ? [
            {
              id: E2E_USER_ROLE_ID,
              name: "イベント担当",
              permissions: "17592186044416",
            },
          ]
        : []),
    ],
  };
}

/** Bot が作ったスケジュールイベント (scheduled_event_id → ギルド)。テスト実行中だけ持つ */
const scheduledEvents = new Map<string, string>();
let nextScheduledEventId = 1;

export function startDiscordMock(port: number): Promise<Server> {
  const server = createServer((req, res) => {
    // リクエストボディは使わないので読み捨てる (未読のままだと接続の後始末で詰まることがある)
    req.resume();
    const url = new URL(req.url ?? "/", "http://discord.local");
    const auth = req.headers.authorization ?? "";
    const json = (status: number, body: unknown) => {
      res.writeHead(status, { "content-type": "application/json" });
      res.end(JSON.stringify(body));
    };
    const notFound = (message: string, code: number) =>
      json(404, { message, code });

    // テスト専用 (Discord には無い): 「イベントの作成」権限を差し替える (#122)
    const permissionsMatch = /^\/_test\/guilds\/(\d+)\/permissions$/.exec(
      url.pathname,
    );
    if (permissionsMatch && (req.method === "PUT" || req.method === "DELETE")) {
      const guildId = permissionsMatch[1];
      if (!E2E_ALL_GUILDS.some((g) => g.id === guildId)) {
        return notFound("Unknown Guild", 10004);
      }
      if (req.method === "PUT") {
        permissionOverrides.set(guildId, {
          botCreateEvents: url.searchParams.get("bot") === "true",
          userCreateEvents: url.searchParams.get("user") === "true",
        });
      } else {
        permissionOverrides.delete(guildId);
      }
      return json(200, permissionOverrides.get(guildId) ?? null);
    }

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

    // スケジュールイベント (#94)。Bot トークン + 「イベントの作成」権限が必要
    const scheduledMatch =
      /^\/guilds\/(\d+)\/scheduled-events(?:\/(\d+))?$/.exec(url.pathname);
    if (scheduledMatch) {
      if (auth !== `Bot ${E2E_BOT_TOKEN}`) {
        return json(401, { message: "401: Unauthorized", code: 0 });
      }
      const [, guildId, scheduledEventId] = scheduledMatch;
      const guild = currentGuild(guildId);
      if (!guild) {
        return notFound("Unknown Guild", 10004);
      }
      if (!guild.botCreateEvents) {
        return json(403, { message: "Missing Permissions", code: 50013 });
      }
      if (req.method === "POST" && scheduledEventId === undefined) {
        const id = `900000000000000${String(nextScheduledEventId++).padStart(3, "0")}`;
        scheduledEvents.set(id, guildId);
        return json(200, { id, guild_id: guildId });
      }
      if (
        (req.method === "PATCH" || req.method === "DELETE") &&
        scheduledEventId !== undefined
      ) {
        if (scheduledEvents.get(scheduledEventId) !== guildId) {
          return notFound("Unknown Guild Scheduled Event", 180000);
        }
        if (req.method === "DELETE") {
          scheduledEvents.delete(scheduledEventId);
          res.writeHead(204);
          return res.end();
        }
        return json(200, { id: scheduledEventId, guild_id: guildId });
      }
      return json(405, { message: "Method Not Allowed", code: 0 });
    }

    if (req.method !== "GET") {
      return json(405, { message: "Method Not Allowed", code: 0 });
    }

    // Bot 自身のユーザー情報 (#94。api が Bot の権限計算に使う)
    if (url.pathname === "/users/@me") {
      if (auth === `Bot ${E2E_BOT_TOKEN}`) {
        return json(200, { id: E2E_BOT_USER_ID, username: "DisCalendar" });
      }
      return json(401, { message: "401: Unauthorized", code: 0 });
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
      const guild = currentGuild(guildId);
      if (!guild) {
        return notFound("Unknown Guild", 10004);
      }
      if (userId === undefined) {
        return json(200, botGuild(guild));
      }
      // Bot 自身のメンバー情報 (#94。api の bot_create_events が読む)
      if (userId === E2E_BOT_USER_ID) {
        return json(200, {
          user: { id: E2E_BOT_USER_ID, username: "DisCalendar" },
          roles: guild.botCreateEvents ? [E2E_BOT_ROLE_ID] : [],
        });
      }
      if (userId !== E2E_USER.discordId) {
        return notFound("Unknown Member", 10007);
      }
      return json(200, {
        user: { id: E2E_USER.discordId, username: E2E_USER.name },
        roles: guild.userCreateEvents ? [E2E_USER_ROLE_ID] : [],
      });
    }

    return notFound("404: Not Found", 0);
  });

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => resolve(server));
  });
}
