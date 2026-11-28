"use client";

import { useLanguage } from "@/components/language-provider";
import { useCanManageServer } from "@/lib/query/guild";

/**
 * 管理権限を持つメンバーにだけ中身を出す (中身の API も管理権限が無いと 403 になる)。
 * 権限の取得中は何も出さない
 */
export function ManageOnly({
  guildId,
  children,
}: {
  guildId: string;
  children: React.ReactNode;
}) {
  const { t } = useLanguage();
  const canManage = useCanManageServer(guildId);
  if (canManage === undefined)
    return <p className="text-sm">{t("読み込み中…")}</p>;
  if (!canManage)
    return (
      <p className="text-sm text-muted-foreground">
        {t("この項目は管理権限を持つメンバーだけが確認・変更できます")}
      </p>
    );
  return children;
}
