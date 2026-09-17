"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, describeApiError } from "@/lib/api";
import { type PendingAttachment, uploadAttachment } from "@/lib/attachments";
import { queryKeys } from "./keys";

export function useAttachmentLimits(guildId?: string) {
  return useQuery({
    queryKey: queryKeys.attachments.limits(guildId ?? ""),
    queryFn: () => api.attachments.limits(guildId ?? ""),
    enabled: !!guildId,
  });
}

export function useAttachmentQueue(guildId?: string) {
  const queryClient = useQueryClient();
  const [items, setItems] = useState<PendingAttachment[]>([]);
  const [busy, setBusy] = useState(false);
  const itemsRef = useRef(items);
  itemsRef.current = items;
  const controller = useRef<AbortController | null>(null);
  const mounted = useRef(true);
  const cancel = useCallback(() => {
    mounted.current = false;
    controller.current?.abort();
  }, []);
  // 再描画より先に予約IDを記録する。送信直後に閉じても削除対象を取りこぼさない。
  function update(key: string, patch: Partial<PendingAttachment>) {
    itemsRef.current = itemsRef.current.map((item) =>
      item.key === key ? { ...item, ...patch } : item,
    );
    setItems(itemsRef.current);
  }
  async function refresh() {
    if (guildId)
      await queryClient.invalidateQueries({
        queryKey: queryKeys.attachments.all(guildId),
      });
  }
  async function discard(item: PendingAttachment) {
    if (guildId && item.reservation && item.eventId) {
      await api.attachments.remove(guildId, item.eventId, item.reservation.id);
    }
    itemsRef.current = itemsRef.current.filter((row) => row.key !== item.key);
    setItems(itemsRef.current);
    await refresh();
  }
  async function upload(eventId: number) {
    if (!guildId || controller.current || !mounted.current) return false;
    const abort = new AbortController();
    controller.current = abort;
    setBusy(true);
    let succeeded = true;
    try {
      for (const item of [...itemsRef.current]) {
        if (abort.signal.aborted) {
          succeeded = false;
          break;
        }
        try {
          const attachment = await uploadAttachment(
            guildId,
            eventId,
            item,
            abort.signal,
            (patch) => update(item.key, patch),
          );
          queryClient.setQueryData(
            queryKeys.attachments.event(guildId, eventId),
            (old: import("@/lib/api/types").EventAttachment[] | undefined) => [
              ...(old ?? []).filter((row) => row.id !== attachment.id),
              attachment,
            ],
          );
          itemsRef.current = itemsRef.current.filter(
            (row) => row.key !== item.key,
          );
          setItems(itemsRef.current);
        } catch (error) {
          succeeded = false;
          update(item.key, {
            status: "failed",
            error: describeApiError(error),
          });
        }
      }
    } finally {
      controller.current = null;
      setBusy(false);
      await refresh();
    }
    return succeeded;
  }
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      controller.current?.abort();
      // unload中は送信を保証できない。取りこぼした予約はサーバーの24時間回収で処理する。
      for (const item of itemsRef.current) {
        if (guildId && item.reservation && item.eventId) {
          void api.attachments
            .remove(guildId, item.eventId, item.reservation.id)
            .catch(() => {});
        }
      }
    };
  }, [guildId]);
  return { items, setItems, busy, upload, discard, cancel };
}
