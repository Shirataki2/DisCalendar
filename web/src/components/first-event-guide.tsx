"use client";

import { CalendarPlusIcon, SettingsIcon, XIcon } from "lucide-react";
import Link from "next/link";
import { useState } from "react";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import { ROUTES } from "@/lib/site";

interface Props {
  guildId: string;
  /** 予定を作れるか。false なら管理者に頼むよう案内する */
  canEdit: boolean;
  /** 作成ダイアログを既定の値で開く */
  onCreate: () => void;
  /** サーバー設定ダイアログ (通知先チャンネル) を開く */
  onOpenSettings: () => void;
}

/** 閉じたことを覚えておく localStorage のキー (サーバーごと) */
const dismissedKey = (guildId: string) =>
  `discalendar-first-event-guide-dismissed:${guildId}`;

function readDismissed(guildId: string): boolean {
  try {
    return window.localStorage.getItem(dismissedKey(guildId)) === "1";
  } catch {
    // 保存が使えない (プライベートブラウズなど) ときは閉じていない扱いにする
    return false;
  }
}

/**
 * 予定がまだ 1 件もないサーバーで、最初の予定作りを手助けする案内 (#377)。
 *
 * カレンダーの高さを削らないよう、画面の下に浮かぶカードにする (スマホでは横いっぱい、PC では右下)。
 * 閉じるとそのサーバーでは端末ごとに出さなくなる。予定を 1 件作っても (予定の有無を取り直して) 出なくなる。
 * 補足 (通知先・繰り返し・Discord のイベント連携) は高さを取らないよう折りたたむ
 */
export function FirstEventGuide({
  guildId,
  canEdit,
  onCreate,
  onOpenSettings,
}: Props) {
  const { t } = useLanguage();
  // 予定の有無を取得してから (クライアントで) 描画するので、初回から保存値を読んでよい
  const [dismissed, setDismissed] = useState(() => readDismissed(guildId));

  if (dismissed) return null;

  function dismiss() {
    setDismissed(true);
    try {
      window.localStorage.setItem(dismissedKey(guildId), "1");
    } catch {
      // 保存できなくても、このページでは閉じられるようにする
    }
  }

  return (
    <section
      aria-labelledby="first-event-guide-title"
      className="fixed inset-x-3 bottom-3 z-40 rounded-xl border border-indigo-400/40 bg-popover p-4 text-sm text-popover-foreground shadow-xl sm:inset-x-auto sm:right-6 sm:bottom-6 sm:w-96"
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <h2 id="first-event-guide-title" className="font-medium">
            {t("このサーバーにはまだ予定がありません")}
          </h2>
          <p className="mt-1 text-muted-foreground">
            {canEdit
              ? t(
                  "最初の予定を作ると、このサーバーのメンバー全員がカレンダーで確認できます。",
                )
              : t(
                  "このサーバーでは管理権限または指定ロールを持つメンバーが予定を作成できます。予定を追加してほしいときは、サーバーの管理者に頼んでください。",
                )}
          </p>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="-mt-1 -mr-1 size-9 shrink-0"
          aria-label={t("最初の予定の案内を閉じる")}
          title={t("閉じる")}
          onClick={dismiss}
        >
          <XIcon aria-hidden />
        </Button>
      </div>

      {canEdit && (
        <>
          <details className="mt-2 text-xs text-muted-foreground">
            <summary className="cursor-pointer font-medium">
              {t("あわせて設定しておくと便利です")}
            </summary>
            <ul className="mt-1 flex list-disc flex-col gap-1 pl-5">
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
              className="mt-2 inline-block font-medium text-foreground underline underline-offset-4"
            >
              {t("使い方を見る")}
            </Link>
          </details>
          <Button
            type="button"
            onClick={onCreate}
            className="mt-3 min-h-11 w-full rounded-full bg-amber-700 px-5 font-semibold text-white hover:bg-amber-600"
          >
            <CalendarPlusIcon />
            {t("最初の予定を作る")}
          </Button>
        </>
      )}
    </section>
  );
}
