"use client";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api, describeApiError } from "@/lib/api";
import type { ApiEvent, ShareLink } from "@/lib/api/types";

export function EventShareControls({ event }: { event: ApiEvent }) {
  const queryClient = useQueryClient();
  const queryKey = ["event-share", event.guild_id, event.id];
  const [message, setMessage] = useState("");
  const share = useQuery({
    queryKey,
    queryFn: () => api.shares.get(event.guild_id, event.id),
  });
  const mutation = useMutation({
    mutationFn: async (action: "copy" | "revoke") => {
      setMessage("");
      await queryClient.cancelQueries({ queryKey });
      if (action === "revoke") {
        await api.shares.revoke(event.guild_id, event.id);
        queryClient.setQueryData(queryKey, null);
        setMessage("共有リンクを無効化しました");
        return;
      }
      const link = await api.shares.issue(event.guild_id, event.id);
      queryClient.setQueryData<ShareLink>(queryKey, link);
      try {
        await navigator.clipboard.writeText(
          `${window.location.origin}/share/${link.token}`,
        );
        setMessage("共有リンクをコピーしました");
      } catch {
        setMessage(
          "コピーできませんでした。下の URL を選択してコピーしてください",
        );
      }
    },
  });
  const url =
    share.data && typeof window !== "undefined"
      ? `${window.location.origin}/share/${share.data.token}`
      : "";
  return (
    <section
      aria-label="予定の共有"
      className="space-y-2 rounded-lg border p-3"
    >
      <p className="text-sm text-muted-foreground">
        リンクを知っている人は、ログインせずに保存済みの予定とサーバー名を閲覧できます。
      </p>
      <div className="flex flex-wrap gap-2">
        <Button
          type="button"
          variant="outline"
          disabled={share.isPending || share.isError || mutation.isPending}
          onClick={() => mutation.mutate("copy")}
        >
          共有リンクをコピー
        </Button>
        {share.data && (
          <Button
            type="button"
            variant="ghost"
            disabled={mutation.isPending}
            onClick={() => mutation.mutate("revoke")}
          >
            共有リンクを無効化
          </Button>
        )}
      </div>
      {url && (
        <Input
          aria-label="共有リンク URL"
          readOnly
          value={url}
          onFocus={(e) => e.target.select()}
        />
      )}
      {message && (
        <p role="status" className="text-sm">
          {message}
        </p>
      )}
      {(share.error || mutation.error) && (
        <p role="alert" className="text-sm text-destructive">
          {describeApiError(share.error ?? mutation.error)}
        </p>
      )}
    </section>
  );
}
