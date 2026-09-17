"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { api, describeApiError } from "@/lib/api";
import type { EventAttachment } from "@/lib/api/types";
import {
  ATTACHMENT_ACCEPT,
  ATTACHMENT_MAX_FILES,
  attachmentValidation,
  type PendingAttachment,
} from "@/lib/attachments";
import { useAttachmentLimits } from "@/lib/query/attachments";
import { queryKeys } from "@/lib/query/keys";

const sizeText = (size: number) =>
  size < 1024 * 1024
    ? `${Math.ceil(size / 1024)} KiB`
    : `${(size / (1024 * 1024)).toFixed(1)} MiB`;

export function EventAttachments({
  guildId,
  eventId,
  editable = false,
  active = true,
}: {
  guildId: string;
  eventId: number;
  editable?: boolean;
  active?: boolean;
}) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [working, setWorking] = useState<string | null>(null);
  const limits = useAttachmentLimits(active ? guildId : undefined);
  const attachments = useQuery({
    queryKey: queryKeys.attachments.event(guildId, eventId),
    queryFn: ({ signal }) => api.attachments.list(guildId, eventId, signal),
    enabled: active,
  });
  async function remove(file: EventAttachment) {
    if (!window.confirm(`「${file.filename}」を削除しますか？`)) return;
    setError(null);
    setWorking(file.id);
    try {
      await api.attachments.remove(guildId, eventId, file.id);
      queryClient.setQueryData<EventAttachment[]>(
        queryKeys.attachments.event(guildId, eventId),
        (old) => old?.filter((row) => row.id !== file.id),
      );
      await queryClient.invalidateQueries({
        queryKey: queryKeys.attachments.all(guildId),
      });
    } catch (e) {
      setError(describeApiError(e));
    } finally {
      setWorking(null);
    }
  }
  async function download(file: EventAttachment) {
    setError(null);
    setWorking(file.id);
    try {
      const signed = await api.attachments.url(guildId, eventId, file.id);
      const anchor = document.createElement("a");
      anchor.href = signed.url;
      anchor.rel = "noreferrer";
      anchor.download = file.filename;
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
    } catch (e) {
      setError(describeApiError(e));
    } finally {
      setWorking(null);
    }
  }
  return (
    <section aria-label="添付ファイル" className="space-y-2 text-sm">
      <p className="font-medium">添付ファイル</p>
      {limits.data && !limits.data.enabled && (
        <p>添付ファイルは現在利用できません</p>
      )}
      {attachments.isPending && (
        <p className="text-muted-foreground">読み込み中…</p>
      )}
      {attachments.error && (
        <p role="alert">
          {describeApiError(attachments.error)}{" "}
          <Button
            type="button"
            variant="link"
            onClick={() => attachments.refetch()}
          >
            再読み込み
          </Button>
        </p>
      )}
      {attachments.data?.length === 0 && (
        <p className="text-muted-foreground">添付ファイルはありません</p>
      )}
      <ul className="max-h-72 space-y-3 overflow-y-auto">
        {attachments.data?.map((file) => (
          <li key={file.id} className="space-y-1 rounded-md border p-2">
            {active &&
              limits.data?.enabled &&
              file.content_type.startsWith("image/") && (
                <AttachmentPreview
                  guildId={guildId}
                  eventId={eventId}
                  file={file}
                />
              )}
            <p className="break-all">{file.filename}</p>
            <p className="text-xs text-muted-foreground">
              {sizeText(file.size)}
            </p>
            <div className="flex flex-wrap gap-1">
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={working !== null || !limits.data?.enabled}
                onClick={() => download(file)}
              >
                ダウンロード
              </Button>
              {editable && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={working !== null}
                  onClick={() => remove(file)}
                >
                  添付を削除
                </Button>
              )}
            </div>
          </li>
        ))}
      </ul>
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
    </section>
  );
}

function AttachmentPreview({
  guildId,
  eventId,
  file,
}: {
  guildId: string;
  eventId: number;
  file: EventAttachment;
}) {
  const [failed, setFailed] = useState(false);
  const url = useQuery({
    queryKey: ["attachment-preview", guildId, eventId, file.id],
    queryFn: () => api.attachments.url(guildId, eventId, file.id, true),
    staleTime: 0,
    gcTime: 0,
    refetchInterval: 240_000,
    retry: false,
  });
  return (
    <>
      {url.data && !failed && (
        // biome-ignore lint/performance/noImgElement: 非公開の期限付きURLをNextの画像最適化キャッシュに保存しない
        <img
          src={url.data.url}
          alt={file.filename}
          loading="lazy"
          referrerPolicy="no-referrer"
          className="max-h-40 w-full rounded object-contain"
          onError={() => {
            if (Date.parse(url.data.expires_at) <= Date.now())
              void url.refetch();
            else setFailed(true);
          }}
        />
      )}
      {(failed || url.error) && (
        <Button
          type="button"
          variant="link"
          size="sm"
          onClick={() => {
            setFailed(false);
            void url.refetch();
          }}
        >
          画像を再読み込み
        </Button>
      )}
    </>
  );
}

export function AttachmentPicker({
  guildId,
  eventId,
  items,
  onChange,
  busy,
  onRetry,
  onRemove,
}: {
  guildId: string;
  eventId?: number;
  items: PendingAttachment[];
  onChange: (items: PendingAttachment[]) => void;
  busy: boolean;
  onRetry: () => void;
  onRemove: (item: PendingAttachment) => Promise<void>;
}) {
  const limits = useAttachmentLimits(guildId);
  const [error, setError] = useState<string | null>(null);
  const existing = useQuery({
    queryKey: queryKeys.attachments.event(guildId, eventId ?? 0),
    queryFn: () => api.attachments.list(guildId, eventId ?? 0),
    enabled: !!eventId,
  });
  function select(files: FileList | null) {
    setError(null);
    if (!files) return;
    const selected = Array.from(files);
    const validation = selected.map(attachmentValidation).find(Boolean);
    if (validation) {
      setError(validation);
      return;
    }
    if (
      items.length + selected.length + (existing.data?.length ?? 0) >
      ATTACHMENT_MAX_FILES
    ) {
      setError("添付は予定ごとに10件までです");
      return;
    }
    onChange([
      ...items,
      ...selected.map(
        (file): PendingAttachment => ({
          key: crypto.randomUUID(),
          file,
          status: "waiting",
        }),
      ),
    ]);
  }
  const status = {
    waiting: "保存後に送信",
    uploading: "送信中…",
    confirming: "確認中…",
    failed: "送信失敗",
  };
  return (
    <section className="space-y-2 text-sm" aria-label="添付の追加">
      <label htmlFor="event-attachments" className="block font-medium">
        ファイルを添付
      </label>
      <input
        id="event-attachments"
        type="file"
        multiple
        accept={ATTACHMENT_ACCEPT}
        disabled={busy || !limits.data?.enabled}
        className="block w-full min-w-0 text-sm"
        onChange={(e) => {
          select(e.target.files);
          e.target.value = "";
        }}
      />
      <p className="text-xs text-muted-foreground">
        JPEG・PNG・WebP・PDF / 1件10MiB、予定ごと10件、サーバー全体1GiBまで
      </p>
      {limits.data && !limits.data.enabled && (
        <p>添付ファイルは現在利用できません</p>
      )}
      {limits.error && (
        <p role="alert">
          {describeApiError(limits.error)}{" "}
          <Button type="button" variant="link" onClick={() => limits.refetch()}>
            再読み込み
          </Button>
        </p>
      )}
      {limits.data?.enabled && (
        <p className="text-xs text-muted-foreground">
          サーバー使用量（送信待ちを含む）: {sizeText(limits.data.used_bytes)} /
          1 GiB
        </p>
      )}
      <ul className="space-y-2" aria-live="polite">
        {items.map((item) => (
          <li key={item.key} className="break-all rounded border p-2">
            {item.file.name} · {sizeText(item.file.size)} ·{" "}
            {status[item.status]}
            {item.error && (
              <p role="alert" className="text-destructive">
                {item.error}
              </p>
            )}
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => {
                void onRemove(item).catch((e) => setError(describeApiError(e)));
              }}
            >
              選択を解除
            </Button>
          </li>
        ))}
      </ul>
      {eventId && items.some((item) => item.status === "failed") && (
        <Button
          type="button"
          variant="outline"
          disabled={busy}
          onClick={onRetry}
        >
          添付を再試行
        </Button>
      )}
      {error && (
        <p role="alert" className="text-destructive">
          {error}
        </p>
      )}
    </section>
  );
}
