import type { BetterAuthPlugin } from "better-auth";
import { APIError, createAuthEndpoint } from "better-auth/api";
import { getOAuth2Tokens, handleOAuthUserInfo } from "better-auth/oauth2";
import { z } from "zod";
import { signSessionToken } from "./session-token";

// E2E では Discord モックに向ける (lib/discord.ts と同じ扱い)
const DISCORD_API =
  process.env.DISCORD_API_BASE_URL ?? "https://discord.com/api/v10";

interface DiscordProfile {
  id: string;
  username: string;
  global_name?: string | null;
  avatar: string | null;
  discriminator?: string;
  email?: string | null;
  verified?: boolean;
}

/** Better Auth の Discord プロバイダ (getUserInfo) と同じ規則でアバター URL を作る */
function avatarUrl(profile: DiscordProfile): string {
  if (profile.avatar === null) {
    const index =
      !profile.discriminator || profile.discriminator === "0"
        ? Number(BigInt(profile.id) >> BigInt(22)) % 6
        : Number.parseInt(profile.discriminator, 10) % 5;
    return `https://cdn.discordapp.com/embed/avatars/${index}.png`;
  }
  const format = profile.avatar.startsWith("a_") ? "gif" : "png";
  return `https://cdn.discordapp.com/avatars/${profile.id}/${profile.avatar}.${format}`;
}

/**
 * Discord の Activity (Embedded App SDK) からのログイン (#350)。
 *
 * Activity は `<client_id>.discordsays.com` の iframe で動き、第三者扱いのため Better Auth の cookie が届かない。
 * そこで SDK の `authorize` で得たコードをここで受け取り、
 * 1. Discord のトークンに交換し (Activity の authorize は redirect_uri を使わないので自前で交換する)
 * 2. 通常の Discord ログインと同じ規則 (handleOAuthUserInfo) でユーザー・account を作成/更新してセッションを作り
 * 3. cookie は付けずに、署名付きのセッショントークンを JSON で返す
 * ブラウザはこの値を `Authorization: Bearer` で api に送る (api は cookie と同じ検証をする。認可は変わらない)
 */
export function discordActivity() {
  return {
    id: "discord-activity",
    endpoints: {
      discordActivitySignIn: createAuthEndpoint(
        "/discord-activity/sign-in",
        {
          method: "POST",
          body: z.object({ code: z.string().min(1).max(512) }),
        },
        async (ctx) => {
          const clientId = process.env.DISCORD_CLIENT_ID;
          const clientSecret = process.env.DISCORD_CLIENT_SECRET;
          if (!clientId || !clientSecret) {
            throw new APIError("SERVICE_UNAVAILABLE", {
              message: "Discord client is not configured",
            });
          }

          const tokenRes = await fetch(`${DISCORD_API}/oauth2/token`, {
            method: "POST",
            headers: { "content-type": "application/x-www-form-urlencoded" },
            body: new URLSearchParams({
              grant_type: "authorization_code",
              code: ctx.body.code,
              client_id: clientId,
              client_secret: clientSecret,
            }),
          });
          if (!tokenRes.ok) {
            ctx.context.logger.warn("Discord activity code exchange failed", {
              status: tokenRes.status,
            });
            throw new APIError("UNAUTHORIZED", {
              message: "Invalid authorization code",
            });
          }
          const tokens = getOAuth2Tokens(await tokenRes.json());
          if (!tokens.accessToken) {
            throw new APIError("UNAUTHORIZED", {
              message: "Invalid authorization code",
            });
          }

          const profileRes = await fetch(`${DISCORD_API}/users/@me`, {
            headers: { authorization: `Bearer ${tokens.accessToken}` },
          });
          if (!profileRes.ok) {
            throw new APIError("UNAUTHORIZED", {
              message: "Failed to get user info",
            });
          }
          const profile = (await profileRes.json()) as DiscordProfile;
          // 通常ログインと同じく、メールアドレスの無いアカウントは受け付けない
          if (!profile.id || !profile.email) {
            throw new APIError("UNAUTHORIZED", {
              message: "Failed to get user info",
            });
          }

          const result = await handleOAuthUserInfo(ctx, {
            userInfo: {
              id: profile.id,
              name: profile.global_name || profile.username || "",
              email: profile.email,
              emailVerified: profile.verified ?? false,
              image: avatarUrl(profile),
            },
            account: {
              providerId: "discord",
              accountId: profile.id,
              accessToken: tokens.accessToken,
              refreshToken: tokens.refreshToken,
              accessTokenExpiresAt: tokens.accessTokenExpiresAt,
              scope: tokens.scopes?.join(","),
            },
            source: {
              method: "oauth",
              oauth: { providerId: "discord", profile: { ...profile } },
            },
          });
          if (result.error !== null) {
            ctx.context.logger.warn("Discord activity sign-in failed", {
              error: result.error,
            });
            throw new APIError("UNAUTHORIZED", { message: result.error });
          }

          return ctx.json({
            token: signSessionToken(
              result.data.session.token,
              ctx.context.secret,
            ),
            expiresAt: result.data.session.expiresAt,
          });
        },
      ),
    },
  } satisfies BetterAuthPlugin;
}

/** Activity のプロキシ (`https://<client_id>.discordsays.com`) からの要求を受け付けるオリジン */
export function activityTrustedOrigins(): string[] {
  const clientId = process.env.DISCORD_CLIENT_ID;
  return clientId ? [`https://${clientId}.discordsays.com`] : [];
}
