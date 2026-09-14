"use client";

import { useState } from "react";
import { McpForm } from "@/components/mcp-form";
import { Button } from "@/components/ui/button";
import { MCP_SCOPES } from "@/lib/mcp/config";

export function McpConsentForm({
  query,
  guilds,
}: {
  query: string;
  guilds: { id: string; name: string }[];
}) {
  const requested = (new URLSearchParams(query).get("scope") ?? "").split(" ");
  const scopes = Object.entries(MCP_SCOPES).filter(([scope]) =>
    requested.includes(scope),
  );
  const [selectedScopes, setScopes] = useState(scopes.map(([scope]) => scope));
  const [selectedGuilds, setGuilds] = useState(guilds.map((guild) => guild.id));
  const groups = [
    {
      name: "scope",
      title: "許可する操作",
      options: scopes.map(([id, name]) => ({ id, name })),
      selected: selectedScopes,
      set: setScopes,
    },
    {
      name: "guild_id",
      title: "許可するサーバー",
      options: guilds,
      selected: selectedGuilds,
      set: setGuilds,
    },
  ];
  return (
    <McpForm action="/mcp/consent/submit">
      <input type="hidden" name="oauth_query" value={query} />
      <p className="text-sm text-muted-foreground">
        要求された操作と現在利用できるサーバーをすべて選択しています。不要な項目を外してください。「選択した内容を許可」を押すまで接続は許可されません。
      </p>
      {groups.map((group) => (
        <fieldset
          key={group.name}
          className="min-w-0 rounded-xl border border-border p-4"
        >
          <legend className="px-1 font-semibold">{group.title}</legend>
          <div className="mb-3 flex flex-wrap items-center gap-2">
            <p role="status" className="mr-auto text-sm text-muted-foreground">
              {group.selected.length} / {group.options.length} 件選択
            </p>
            <Button
              type="button"
              variant="outline"
              disabled={!group.options.length}
              onClick={() =>
                group.set(group.options.map((option) => option.id))
              }
            >
              すべて選択
            </Button>
            <Button
              type="button"
              variant="outline"
              disabled={!group.selected.length}
              onClick={() => group.set([])}
            >
              すべて解除
            </Button>
          </div>
          <div className="space-y-1">
            {group.options.map((option) => (
              <label
                key={option.id}
                className="flex min-h-11 cursor-pointer items-center gap-3 rounded-lg px-2 py-2 hover:bg-muted"
              >
                <input
                  type="checkbox"
                  name={group.name}
                  value={option.id}
                  checked={group.selected.includes(option.id)}
                  className="size-4 shrink-0 accent-primary focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-ring"
                  onChange={(event) =>
                    group.set(
                      event.target.checked
                        ? [...group.selected, option.id]
                        : group.selected.filter((id) => id !== option.id),
                    )
                  }
                />
                <span className="min-w-0 break-words text-sm">
                  {option.name}
                </span>
              </label>
            ))}
          </div>
          {!group.options.length && (
            <p className="text-sm text-muted-foreground">
              {group.name === "guild_id"
                ? "許可できるサーバーがありません。本人とBotが参加しているサーバーを確認してください。"
                : "許可できる操作が要求されていません。クライアントから接続をやり直してください。"}
            </p>
          )}
        </fieldset>
      ))}
      {(!selectedScopes.length || !selectedGuilds.length) && (
        <p role="status" className="text-sm text-muted-foreground">
          許可する操作とサーバーをそれぞれ1件以上選んでください。拒否は選択なしでも行えます。
        </p>
      )}
      {selectedGuilds.length > 100 && (
        <p role="status">
          サーバーは100件まで許可できます。不要なサーバーを外してください。
        </p>
      )}
      <p className="text-sm text-muted-foreground">
        後から参加するサーバーは追加されません。接続管理からいつでも解除できます。
      </p>
      <div className="flex flex-wrap gap-3">
        <Button
          type="submit"
          name="accept"
          value="true"
          size="lg"
          disabled={
            !selectedScopes.length ||
            !selectedGuilds.length ||
            selectedGuilds.length > 100
          }
        >
          選択した内容を許可
        </Button>
        <Button
          type="submit"
          name="accept"
          value="false"
          size="lg"
          variant="outline"
        >
          拒否
        </Button>
      </div>
    </McpForm>
  );
}
