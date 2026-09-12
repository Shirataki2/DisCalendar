"use client";

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useFormContext, useWatch } from "react-hook-form";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api, describeApiError } from "@/lib/api";
import type { NotificationMention } from "@/lib/api/types";
import {
  type EventFormValues,
  MENTIONS_MAX,
  mentionIdSchema,
} from "@/lib/event-form";
import { useGuildRolesQuery, useMyPermissionsQuery } from "@/lib/query/guild";

export function NotificationMentionsField({ guildId }: { guildId: string }) {
  const {
    control,
    setValue,
    formState: { errors, isSubmitting },
  } = useFormContext<EventFormValues>();
  const mentions = useWatch({ control, name: "notificationMentions" }) ?? [];
  const roles = useGuildRolesQuery(guildId, true);
  const permissions = useMyPermissionsQuery(guildId);
  const canMentionEveryone =
    permissions.data?.administrator ||
    (BigInt(permissions.data?.permissions ?? "0") & BigInt(131072)) !==
      BigInt(0);
  const [userId, setUserId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const userIds = mentions.flatMap((m) => (m.type === "user" ? [m.id] : []));
  const profiles = useQuery({
    queryKey: ["guild", guildId, "mention-members", userIds],
    queryFn: ({ signal }) =>
      api.guilds.mentionMembers(guildId, userIds, signal),
    enabled: userIds.length > 0,
    retry: false,
  });
  const update = (next: NotificationMention[]) =>
    setValue("notificationMentions", next, {
      shouldDirty: true,
      shouldValidate: true,
    });
  const add = (mention: NotificationMention) => {
    if (mentions.length >= MENTIONS_MAX) return;
    if (
      mentions.some(
        (m) =>
          m.type === mention.type &&
          (m.type === "everyone" ||
            (mention.type !== "everyone" && m.id === mention.id)),
      )
    )
      return;
    update([...mentions, mention]);
  };
  const disabled = isSubmitting || adding;
  const full = mentions.length >= MENTIONS_MAX;
  const roleOptions =
    roles.data?.filter(
      (role) =>
        (role.mentionable || canMentionEveryone) &&
        !mentions.some((m) => m.type === "role" && m.id === role.id),
    ) ?? [];

  return (
    <Field>
      <FieldLabel>通知のメンション先</FieldLabel>
      <FieldDescription>
        事前通知・開始時刻の通知で呼びかける相手を、合計10件まで指定できます。
        Bot に権限がない場合、通知は届きますが @everyone
        やメンション不可のロールへの呼びかけは届きません。
      </FieldDescription>
      <label
        htmlFor="mention-everyone"
        className="flex items-center gap-2 text-sm"
      >
        <Checkbox
          id="mention-everyone"
          checked={mentions.some((m) => m.type === "everyone")}
          disabled={
            disabled ||
            (!mentions.some((m) => m.type === "everyone") &&
              (full || !canMentionEveryone))
          }
          onCheckedChange={(checked) =>
            checked
              ? add({ type: "everyone" })
              : update(mentions.filter((m) => m.type !== "everyone"))
          }
        />
        @everyone（全員）
      </label>
      {!canMentionEveryone && (
        <FieldDescription>
          全員やメンション不可のロールを指定するには、あなたに「全てのロールにメンション」権限が必要です。
        </FieldDescription>
      )}
      <Select
        value={null}
        items={roleOptions.map((role) => ({
          value: role.id,
          label: role.name,
        }))}
        disabled={disabled || full || roleOptions.length === 0}
        onValueChange={(id) => {
          if (id) add({ type: "role", id });
        }}
      >
        <SelectTrigger aria-label="メンションするロール">
          <SelectValue
            placeholder={
              roles.isPending ? "ロールを読み込み中…" : "ロールを追加"
            }
          />
        </SelectTrigger>
        <SelectContent>
          {roleOptions.map((role) => (
            <SelectItem key={role.id} value={role.id}>
              {role.name}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {roles.isError && (
        <FieldError>
          ロールを取得できませんでした。
          <button
            type="button"
            className="underline"
            onClick={() => roles.refetch()}
          >
            再試行
          </button>
        </FieldError>
      )}
      <div className="flex gap-2">
        <Input
          aria-label="メンションするユーザーID"
          placeholder="Discord のユーザーID"
          value={userId}
          disabled={disabled || full}
          onChange={(event) => setUserId(event.target.value)}
        />
        <Button
          type="button"
          variant="outline"
          disabled={disabled || full || !userId.trim()}
          onClick={async () => {
            setError(null);
            const parsed = mentionIdSchema.safeParse(userId.trim());
            if (!parsed.success) {
              setError("正しいユーザーIDを入力してください");
              return;
            }
            setAdding(true);
            try {
              const [profile] = await api.guilds.mentionMembers(guildId, [
                parsed.data,
              ]);
              if (!profile?.display_name) {
                setError("このユーザーはサーバーに参加していません");
                return;
              }
              add({ type: "user", id: parsed.data });
              setUserId("");
            } catch (cause) {
              setError(describeApiError(cause));
            } finally {
              setAdding(false);
            }
          }}
        >
          {adding ? "確認中…" : "ユーザーを追加"}
        </Button>
      </div>
      <FieldDescription>
        ユーザーIDは Discord の「設定 → 詳細設定 →
        開発者モード」を有効にし、相手のメニューから「ユーザーIDをコピー」で取得できます。
      </FieldDescription>
      {mentions.length > 0 && (
        <ul className="flex flex-col gap-1 text-sm">
          {mentions.map((mention, index) => {
            const label =
              mention.type === "everyone"
                ? "@everyone"
                : mention.type === "role"
                  ? `@${roles.data?.find((r) => r.id === mention.id)?.name ?? mention.id}`
                  : `@${profiles.data?.find((p) => p.user_id === mention.id)?.display_name ?? mention.id}`;
            return (
              <li
                key={
                  mention.type === "everyone"
                    ? mention.type
                    : `${mention.type}:${mention.id}`
                }
                className="flex items-center justify-between gap-2"
              >
                <span className="break-all">{label}</span>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  disabled={disabled}
                  aria-label={`${label}のメンションを削除`}
                  onClick={() => update(mentions.filter((_, i) => i !== index))}
                >
                  削除
                </Button>
              </li>
            );
          })}
        </ul>
      )}
      <FieldError>{error ?? errors.notificationMentions?.message}</FieldError>
    </Field>
  );
}
