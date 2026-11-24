"use client";

import { HeartIcon, XIcon } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { useLanguage } from "@/components/language-provider";
import { Button } from "@/components/ui/button";
import {
  dismissDonationNotice,
  shouldShowDonationNotice,
} from "@/lib/donation-notice";
import { ROUTES } from "@/lib/site";

/**
 * ある程度使ってくれている人に一度だけ出す、支援ページへの控えめなお知らせ (#371)。
 *
 * 表示条件と保存先は lib/donation-notice.ts。カレンダーの操作を邪魔しないよう、
 * 最初の予定の案内 (first-event-guide.tsx) と同じく画面の下に浮かぶ小さなカードにする。
 * 閉じる・支援ページを開くのどちらでも二度と出さない
 */
export function DonationNotice() {
  const { t } = useLanguage();
  // localStorage はサーバーで読めないので、hydration の後に読む (初回の描画では出さない)
  const [show, setShow] = useState(false);
  useEffect(() => {
    setShow(shouldShowDonationNotice());
  }, []);

  if (!show) return null;

  function dismiss() {
    setShow(false);
    dismissDonationNotice();
  }

  return (
    <section
      aria-labelledby="donation-notice-title"
      className="fixed inset-x-3 bottom-3 z-40 rounded-xl border border-indigo-400/40 bg-popover p-4 text-sm text-popover-foreground shadow-xl sm:inset-x-auto sm:right-6 sm:bottom-6 sm:w-96"
    >
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <h2 id="donation-notice-title" className="font-medium">
            {t("いつも DisCalendar をご利用いただきありがとうございます")}
          </h2>
          <p className="mt-1 text-muted-foreground">
            {t(
              "DisCalendar は無料で使えます。気に入っていただけたら、任意の支援で運営を応援していただけると励みになります。このお知らせは一度だけ表示します。",
            )}
          </p>
          <Link
            href={ROUTES.donation}
            onClick={() => dismissDonationNotice()}
            className="mt-2 inline-flex items-center gap-1 font-medium text-foreground underline underline-offset-4"
          >
            <HeartIcon className="size-3" aria-hidden />
            {t("支援ページを見る")}
          </Link>
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="-mt-1 -mr-1 size-9 shrink-0"
          aria-label={t("支援のお知らせを閉じる")}
          title={t("閉じる")}
          onClick={dismiss}
        >
          <XIcon aria-hidden />
        </Button>
      </div>
    </section>
  );
}
