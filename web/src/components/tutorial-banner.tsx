"use client";

import { XIcon } from "lucide-react";
import Link from "next/link";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { ROUTES } from "@/lib/site";

const STORAGE_KEY = "discalendar-tutorial-dismissed";

export function TutorialBanner({ highlighted }: { highlighted: boolean }) {
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    try {
      setVisible(localStorage.getItem(STORAGE_KEY) !== "1");
    } catch {
      setVisible(true);
    }
  }, []);

  if (!visible) return null;

  function dismiss() {
    setVisible(false);
    try {
      localStorage.setItem(STORAGE_KEY, "1");
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
