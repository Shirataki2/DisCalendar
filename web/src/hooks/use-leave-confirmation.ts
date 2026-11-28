"use client";

import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

/**
 * 未保存の変更があるあいだ、ページを離れる前に確認する (#440)。
 *
 * - アプリ内のリンク (`<a href>`。next/link を含む) で別のページへ移るときは止めて、`pendingHref` に行き先を入れる。
 *   呼び出し側が確認ダイアログを出し、`leave()` で移動、`stay()` でとどまる
 * - 再読み込み・タブを閉じる・外部サイトへの移動は、ブラウザ標準の確認 (`beforeunload`) に任せる
 *
 * App Router には遷移を止める API が無いので、document のキャプチャ段階でクリックを拾い、
 * next/link の onClick (React のルートで受ける) より先に止める。ブラウザの戻る・進むは対象外
 */
export function useLeaveConfirmation(dirty: boolean) {
  const router = useRouter();
  const [pendingHref, setPendingHref] = useState<string | null>(null);

  useEffect(() => {
    if (!dirty) return;
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      // 古いブラウザ向け (値は表示されず、標準の文言が出る)
      event.returnValue = "";
    };
    const onClick = (event: MouseEvent) => {
      const href = interceptedHref(event);
      if (href === null) return;
      event.preventDefault();
      event.stopPropagation();
      setPendingHref(href);
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    document.addEventListener("click", onClick, true);
    return () => {
      window.removeEventListener("beforeunload", onBeforeUnload);
      document.removeEventListener("click", onClick, true);
    };
  }, [dirty]);

  const leave = useCallback(() => {
    const href = pendingHref;
    setPendingHref(null);
    if (href) router.push(href);
  }, [pendingHref, router]);
  const stay = useCallback(() => setPendingHref(null), []);

  return { pendingHref, leave, stay };
}

/**
 * 止めるべきクリックなら行き先 (同じオリジンのパス + クエリ + ハッシュ) を返す。
 * 新しいタブで開く操作・別オリジン・ダウンロード・同じページ内のアンカーは止めない
 */
function interceptedHref(event: MouseEvent): string | null {
  if (
    event.defaultPrevented ||
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  )
    return null;
  if (!(event.target instanceof Element)) return null;
  const anchor = event.target.closest("a[href]");
  if (!(anchor instanceof HTMLAnchorElement)) return null;
  if (anchor.target && anchor.target !== "_self") return null;
  if (anchor.hasAttribute("download")) return null;
  const url = new URL(anchor.href, window.location.href);
  // 別オリジンへの移動は beforeunload で確認される
  if (url.origin !== window.location.origin) return null;
  // 同じページ内のアンカー (#...) は変更を失わない
  if (
    url.pathname === window.location.pathname &&
    url.search === window.location.search
  )
    return null;
  return `${url.pathname}${url.search}${url.hash}`;
}
