import { ApiError, api } from "@/lib/api";
import type { AttachmentReservation, EventAttachment } from "@/lib/api/types";

export const ATTACHMENT_MAX_BYTES = 10 * 1024 * 1024;
export const ATTACHMENT_MAX_FILES = 10;
export const ATTACHMENT_ACCEPT = ".jpg,.jpeg,.png,.webp,.pdf";

/** OSがMIMEを返さない場合も拡張子から送信形式を決める。実体の検証はAPIが行う。 */
export function attachmentType(filename: string): string | undefined {
  return (
    {
      jpg: "image/jpeg",
      jpeg: "image/jpeg",
      png: "image/png",
      webp: "image/webp",
      pdf: "application/pdf",
    } as Record<string, string>
  )[filename.split(".").pop()?.toLowerCase() ?? ""];
}
export function attachmentValidation(file: File): string | null {
  if (!attachmentType(file.name))
    return "JPEG・PNG・WebP・PDFを選択してください";
  if (!file.size || file.size > ATTACHMENT_MAX_BYTES)
    return "ファイルは1バイト以上10MiB以下にしてください";
  return null;
}

export interface PendingAttachment {
  key: string;
  file: File;
  status: "waiting" | "uploading" | "confirming" | "failed";
  error?: string;
  reservation?: AttachmentReservation;
  eventId?: number;
}

/** 完了応答を失っても同じ予約を確定する。PUTの再送はIf-None-Matchで上書きしない。 */
export async function uploadAttachment(
  guildId: string,
  eventId: number,
  item: PendingAttachment,
  signal: AbortSignal,
  update: (patch: Partial<PendingAttachment>) => void,
): Promise<EventAttachment> {
  let reservation = item.reservation;
  if (reservation) {
    update({ status: "confirming" });
    try {
      return await api.attachments.complete(
        guildId,
        eventId,
        reservation.id,
        signal,
      );
    } catch (error) {
      if (
        signal.aborted ||
        (error instanceof ApiError && [401, 403].includes(error.status))
      )
        throw error;
      // 期限切れ・内容不正の予約は捨て、新しいキーへ送る。通信障害だけなら後から同じ予約で再試行する。
      if (
        Date.parse(reservation.upload.expires_at) <= Date.now() ||
        (error instanceof ApiError && [400, 404].includes(error.status))
      ) {
        await api.attachments.remove(guildId, eventId, reservation.id);
        reservation = undefined;
        update({ reservation: undefined });
      }
    }
  }
  if (!reservation) {
    update({ status: "uploading" });
    reservation = await api.attachments.reserve(
      guildId,
      eventId,
      {
        filename: item.file.name,
        size: item.file.size,
        content_type: attachmentType(item.file.name) ?? "",
      },
      signal,
    );
    update({ reservation, eventId });
  }
  update({ status: "uploading" });
  const response = await fetch(reservation.upload.url, {
    method: "PUT",
    headers: reservation.upload.headers,
    body: item.file,
    signal,
    credentials: "omit",
  });
  // 既に送信済みならそのオブジェクトを検証する。期限切れ等の他の失敗を成功扱いしない。
  if (!response.ok && response.status !== 412)
    throw new Error("ファイルを送信できませんでした");
  update({ status: "confirming" });
  return api.attachments.complete(guildId, eventId, reservation.id, signal);
}
