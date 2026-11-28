import type { GuildApiToken, GuildWebhook } from "@/lib/api/types";

export interface CreatorIntegrations {
  /** 登録した Discord ユーザーの ID (Snowflake のまま文字列で扱う) */
  userId: string;
  webhooks: GuildWebhook[];
  tokens: GuildApiToken[];
}

/**
 * Webhook と API トークンを登録したユーザー (`created_by`) ごとにまとめる。
 * 登録した数が多い人を先に、同数なら最初に登録した人を先に並べる。各ユーザーの中は API の返した順のまま
 */
export function groupIntegrationsByCreator(
  webhooks: readonly GuildWebhook[],
  tokens: readonly GuildApiToken[],
): CreatorIntegrations[] {
  const groups = new Map<
    string,
    CreatorIntegrations & { firstCreatedAt: number }
  >();
  const groupOf = (userId: string, createdAt: string) => {
    const time = new Date(createdAt).getTime();
    let group = groups.get(userId);
    if (!group) {
      group = { userId, webhooks: [], tokens: [], firstCreatedAt: time };
      groups.set(userId, group);
    } else if (time < group.firstCreatedAt) {
      group.firstCreatedAt = time;
    }
    return group;
  };
  for (const hook of webhooks)
    groupOf(hook.created_by, hook.created_at).webhooks.push(hook);
  for (const token of tokens)
    groupOf(token.created_by, token.created_at).tokens.push(token);
  return [...groups.values()]
    .sort(
      (a, b) =>
        b.webhooks.length +
          b.tokens.length -
          (a.webhooks.length + a.tokens.length) ||
        a.firstCreatedAt - b.firstCreatedAt,
    )
    .map(({ firstCreatedAt: _, ...group }) => group);
}
