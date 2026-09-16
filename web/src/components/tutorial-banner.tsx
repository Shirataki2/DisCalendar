"use client";

import { XIcon } from "lucide-react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { ROUTES } from "@/lib/site";

export function TutorialBanner({
  highlighted,
  defaultVisible,
}: {
  highlighted: boolean;
  defaultVisible: boolean;
}) {
  const router = useRouter();
  const [visible, setVisible] = useState(defaultVisible);

  if (!visible) return null;

  function dismiss() {
    setVisible(false);
    try {
      // biome-ignore lint/suspicious/noDocumentCookie: Cookie Store API は Safari / Firefox の少し前の版に無いので document.cookie を使う
      document.cookie =
        "discalendar-tutorial-dismissed=1; path=/; max-age=31536000; samesite=lax";
      // 戻る操作でも、閉じる前のサーバー描画を再利用しないようにする。
      router.refresh();
    } catch {
      // 保存が許可されていない場合も、このページでは閉じられるようにする。
    }
  }

  return (
    <div
      className={`mb-6 flex items-start rounded-lg border ${highlighted ? "border-indigo-400/40 bg-indigo-500/10" : "border-border"}`}
    >
      <Link
        href={ROUTES.tutorial}
        className="min-w-0 flex-1 rounded-lg px-4 py-3 hover:bg-muted"
      >
        <span className="font-semibold">練習用カレンダーで操作を試す</span>
        <span className="mt-1 block text-sm text-muted-foreground">
          Botの導入や編集権限がなくても、予定の作成から通知まで体験できます。
        </span>
      </Link>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        className="m-2 size-11"
        aria-label="練習用カレンダーの案内を閉じる"
        onClick={dismiss}
      >
        <XIcon aria-hidden />
      </Button>
    </div>
  );
}
