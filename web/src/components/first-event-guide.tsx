"use client";

import { CalendarPlusIcon, SettingsIcon } from "lucide-react";
import Link from "next/link";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { ROUTES } from "@/lib/site";

interface Props {
  /** 予定を作れるか。false なら管理者に頼むよう案内する */
  canEdit: boolean;
  /** 作成ダイアログを既定の値で開く */
  onCreate: () => void;
  /** サーバー設定ダイアログ (通知先チャンネル) を開く */
  onOpenSettings: () => void;
}

/**
 * 予定がまだ 1 件もないサーバーで、最初の予定作りを手助けする案内 (#377)。
 * 導入したあと最初の予定を作る前に離れてしまうのを減らすため、カレンダーの上に出す。
 * 予定を 1 件作ると (予定の有無を取り直して) 出なくなる
 */
export function FirstEventGuide({ canEdit, onCreate, onOpenSettings }: Props) {
  const { t } = useLanguage();

  if (!canEdit) {
    return (
      <section
        aria-labelledby="first-event-guide-title"
        className="rounded-lg border border-indigo-400/30 bg-indigo-500/10 px-4 py-3 text-sm"
      >
        <h2 id="first-event-guide-title" className="font-medium">
          {t("このサーバーにはまだ予定がありません")}
        </h2>
        <p className="mt-1 text-muted-foreground">
          {t(
            "このサーバーでは管理権限または指定ロールを持つメンバーが予定を作成できます。予定を追加してほしいときは、サーバーの管理者に頼んでください。",
          )}
        </p>
      </section>
    );
  }

  return (
    <section
      aria-labelledby="first-event-guide-title"
      className="flex flex-col gap-3 rounded-lg border border-indigo-400/30 bg-indigo-500/10 px-4 py-3 text-sm sm:flex-row sm:items-start sm:justify-between"
    >
      <div className="flex flex-col gap-2">
        <div>
          <h2 id="first-event-guide-title" className="font-medium">
            {t("このサーバーにはまだ予定がありません")}
          </h2>
          <p className="mt-1 text-muted-foreground">
            {t(
              "最初の予定を作ると、このサーバーのメンバー全員がカレンダーで確認できます。",
            )}
          </p>
        </div>
        <div>
          <p className="text-xs font-medium text-muted-foreground">
            {t("あわせて設定しておくと便利です")}
          </p>
          <ul className="mt-1 flex list-disc flex-col gap-1 pl-5 text-xs text-muted-foreground">
            <li>
              {t(
                "通知先チャンネル: 予定の開始時刻や事前通知を Discord のチャンネルに投稿します。",
              )}
              <button
                type="button"
                onClick={onOpenSettings}
                className="ml-1 inline-flex items-center gap-1 font-medium text-foreground underline underline-offset-4"
              >
                <SettingsIcon className="size-3" aria-hidden />
                {t("通知先を設定する")}
              </button>
            </li>
            <li>
              {t(
                "繰り返し予定: 毎週・毎月の予定は、作成画面の「繰り返しの設定」で一度に登録できます。",
              )}
            </li>
            <li>
              {t(
                "Discord のイベント連携: 作成画面で「Discord のイベントとしても作成する」を選ぶと、サーバーのイベント一覧にも表示されます。",
              )}
            </li>
          </ul>
          <Link
            href={ROUTES.docs}
            className="mt-2 inline-block text-xs font-medium underline underline-offset-4"
          >
            {t("使い方を見る")}
          </Link>
        </div>
      </div>
      <Button
        type="button"
        onClick={onCreate}
        className="min-h-11 shrink-0 rounded-full bg-amber-700 px-5 font-semibold text-white hover:bg-amber-600"
      >
        <CalendarPlusIcon />
        {t("最初の予定を作る")}
      </Button>
    </section>
  );
}
