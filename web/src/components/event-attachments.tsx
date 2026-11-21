"use client";

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { api, describeApiError } from "@/lib/api";
import type { EventAttachment } from "@/lib/api/types";
import {
  ATTACHMENT_ACCEPT,
  ATTACHMENT_MAX_BYTES,
  ATTACHMENT_MAX_FILES,
  attachmentType,
  attachmentValidation,
  coverValidation,
  isImageType,
  type PendingAttachment,
} from "@/lib/attachments";
import { useAttachmentLimits } from "@/lib/query/attachments";
import { queryKeys } from "@/lib/query/keys";

const sizeText = (size: number) =>
  size < 1_000_000
    ? `${Math.ceil(size / 1000)} KB`
    : `${(size / 1_000_000).toFixed(1)} MB`;

/** Discord イベントのカバーの選択 (#403)。保存済みの添付では添付の ID、送信前ではキーを渡す */
export interface CoverChoice {
  /** 選択肢を出すか (Discord 連携がオンで、繰り返し予定でない) */
  selectable: boolean;
  /** カバーの保存中 */
  busy?: boolean;
  onChange: (value: string | null) => void;
  /** 添付を削除した (カバーの失敗表示を取り下げるのに使う) */
  onRemoved?: (file: EventAttachment) => void;
}

export function EventAttachments({
  guildId,
  eventId,
  editable = false,
  active = true,
  cover,
}: {
  guildId: string;
  eventId: number;
  editable?: boolean;
  active?: boolean;
  cover?: CoverChoice;
}) {
  const { t, language } = useLanguage();
  const queryClient = useQueryClient();
  const [error, setError] = useState<unknown>(null);
  const [working, setWorking] = useState<string | null>(null);
  const limits = useAttachmentLimits(active ? guildId : undefined);
  const attachments = useQuery({
    queryKey: queryKeys.attachments.event(guildId, eventId),
    queryFn: ({ signal }) => api.attachments.list(guildId, eventId, signal),
    enabled: active,
  });
  async function remove(file: EventAttachment) {
    if (
      !window.confirm(
        t("「{filename}」を削除しますか？", { filename: file.filename }),
      )
    )
      return;
    setError(null);
    setWorking(file.id);
    try {
      await api.attachments.remove(guildId, eventId, file.id);
      cover?.onRemoved?.(file);
      queryClient.setQueryData<EventAttachment[]>(
        queryKeys.attachments.event(guildId, eventId),
        (old) => old?.filter((row) => row.id !== file.id),
      );
      await queryClient.invalidateQueries({
        queryKey: queryKeys.attachments.all(guildId),
      });
    } catch (e) {
      setError(e);
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
      setError(e);
    } finally {
      setWorking(null);
    }
  }
  return (
    <section aria-label={t("添付ファイル")} className="space-y-2 text-sm">
      <p className="font-medium">{t("添付ファイル")}</p>
      {limits.data && !limits.data.enabled && (
        <p>{t("添付ファイルは現在利用できません")}</p>
      )}
      {attachments.isPending && (
        <p className="text-muted-foreground">{t("読み込み中…")}</p>
      )}
      {attachments.error && (
        <p role="alert">
          {describeApiError(attachments.error, language)}{" "}
          <Button
            type="button"
            variant="link"
            onClick={() => attachments.refetch()}
          >
            {t("再読み込み")}
          </Button>
        </p>
      )}
      {attachments.data?.length === 0 && (
        <p className="text-muted-foreground">{t("添付ファイルはありません")}</p>
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
            <p className="break-all">
              {file.filename}
              {file.discord_cover && (
                <span className="ml-2 rounded-sm bg-primary/10 px-1.5 py-0.5 text-xs text-primary">
                  {t("Discord のカバー")}
                </span>
              )}
            </p>
            <p className="text-xs text-muted-foreground">
              {sizeText(file.size)}
            </p>
            {editable &&
              cover?.selectable &&
              isImageType(file.content_type) &&
              coverValidation(file) && (
                <p className="text-xs text-muted-foreground">
                  {t(coverValidation(file) ?? "")}
                </p>
              )}
            <div className="flex flex-wrap gap-1">
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={working !== null || !limits.data?.enabled}
                onClick={() => download(file)}
              >
                {t("ダウンロード")}
              </Button>
              {editable &&
                cover?.selectable &&
                isImageType(file.content_type) && (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={
                      working !== null ||
                      !!cover.busy ||
                      (!file.discord_cover && !!coverValidation(file))
                    }
                    aria-pressed={file.discord_cover}
                    onClick={() =>
                      cover.onChange(file.discord_cover ? null : file.id)
                    }
                  >
                    {file.discord_cover
                      ? t("カバーを外す")
                      : t("Discord のカバーにする")}
                  </Button>
                )}
              {editable && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={working !== null}
                  onClick={() => remove(file)}
                >
                  {t("添付を削除")}
                </Button>
              )}
            </div>
          </li>
        ))}
      </ul>
      {!!error && (
        <p role="alert" className="text-destructive">
          {typeof error === "string"
            ? t(error)
            : describeApiError(error, language)}
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
  const { t } = useLanguage();
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
          {t("画像を再読み込み")}
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
  cover,
  coverKey = null,
}: {
  guildId: string;
  eventId?: number;
  items: PendingAttachment[];
  onChange: (items: PendingAttachment[]) => void;
  busy: boolean;
  onRetry: () => void;
  onRemove: (item: PendingAttachment) => Promise<void>;
  /** 送信前の添付からカバーを選ぶ (#403)。`coverKey` は選んだ添付のキー */
  cover?: CoverChoice;
  coverKey?: string | null;
}) {
  const { t, language } = useLanguage();
  const limits = useAttachmentLimits(guildId);
  const [error, setError] = useState<unknown>(null);
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
    waiting: t("保存後に送信"),
    uploading: t("送信中…"),
    confirming: t("確認中…"),
    failed: t("送信失敗"),
  };
  return (
    <section className="space-y-2 text-sm" aria-label={t("添付の追加")}>
      <label htmlFor="event-attachments" className="block font-medium">
        {t("ファイルを添付")}
      </label>
      <input
        id="event-attachments"
        type="file"
        multiple
        accept={ATTACHMENT_ACCEPT}
        disabled={busy || !limits.data?.enabled}
        className="block w-full min-w-0 rounded-md text-sm file:mr-3 file:cursor-pointer file:rounded-md file:border-0 file:bg-primary file:px-3 file:py-2 file:text-sm file:font-medium file:text-primary-foreground file:shadow-xs hover:file:bg-primary/90 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:opacity-50"
        onChange={(e) => {
          select(e.target.files);
          e.target.value = "";
        }}
      />
      <p className="text-xs text-muted-foreground">
        {t("JPEG・PNG・WebP・PDF / 1件 約10.5 MB、予定ごと10件まで")}
      </p>
      {cover?.selectable && (
        <p className="text-xs text-muted-foreground">
          {t(
            "画像の 1 枚を Discord のイベントのカバーにできます (JPEG・PNG・WebP / 8 MiB まで。推奨 800×320 の横長)",
          )}
        </p>
      )}
      {limits.data && !limits.data.enabled && (
        <p>{t("添付ファイルは現在利用できません")}</p>
      )}
      {limits.error && (
        <p role="alert">
          {describeApiError(limits.error, language)}{" "}
          <Button type="button" variant="link" onClick={() => limits.refetch()}>
            {t("再読み込み")}
          </Button>
        </p>
      )}
      <details className="text-xs text-muted-foreground">
        <summary className="min-h-11 cursor-pointer content-center rounded-sm focus-visible:outline-2 focus-visible:outline-ring">
          {t("容量制限とサーバー使用量")}
        </summary>
        <p>
          {t(
            "1件 {size} バイト（10 MiB）、サーバー全体 1,073,741,824 バイト（1 GiB）まで。空のファイルは添付できません。",
            { size: ATTACHMENT_MAX_BYTES.toLocaleString(language) },
          )}
        </p>
        {limits.data?.enabled && (
          <p className="mt-1">
            {t("サーバー使用量（送信待ちを含む）:")}{" "}
            {sizeText(limits.data.used_bytes)} {t("/ 約1.07 GB")}
          </p>
        )}
      </details>
      <ul className="space-y-2" aria-live="polite">
        {items.map((item) => (
          <li key={item.key} className="break-all rounded border p-2">
            {item.file.name} · {sizeText(item.file.size)} ·{" "}
            {status[item.status]}
            {coverKey === item.key && (
              <span className="ml-2 rounded-sm bg-primary/10 px-1.5 py-0.5 text-xs text-primary">
                {t("Discord のカバー")}
              </span>
            )}
            {!!item.error && (
              <p role="alert" className="text-destructive">
                {typeof item.error === "string"
                  ? t(item.error)
                  : describeApiError(item.error, language)}
              </p>
            )}
            {cover?.selectable &&
              isImageType(attachmentType(item.file.name)) &&
              (() => {
                const reason = coverValidation({
                  content_type: attachmentType(item.file.name),
                  size: item.file.size,
                });
                return reason ? (
                  <p className="text-xs text-muted-foreground">{t(reason)}</p>
                ) : (
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={busy}
                    aria-pressed={coverKey === item.key}
                    onClick={() =>
                      cover.onChange(coverKey === item.key ? null : item.key)
                    }
                  >
                    {coverKey === item.key
                      ? t("カバーを外す")
                      : t("Discord のカバーにする")}
                  </Button>
                );
              })()}
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={busy}
              onClick={() => {
                if (coverKey === item.key) cover?.onChange(null);
                void onRemove(item).catch((e) => setError(e));
              }}
            >
              {t("選択を解除")}
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
          {t("添付を再試行")}
        </Button>
      )}
      {!!error && (
        <p role="alert" className="text-destructive">
          {typeof error === "string"
            ? t(error)
            : describeApiError(error, language)}
        </p>
      )}
    </section>
  );
}
