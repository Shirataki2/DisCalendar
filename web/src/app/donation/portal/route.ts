import { redirect } from "next/navigation";
import { donationLinkUrl, donationUnavailableResponse } from "@/lib/donation";

// 支援ページ (/donation) の「毎月の支援の解約・カード変更」の飛び先。Stripe カスタマーポータルの
// ログインリンク (STRIPE_CUSTOMER_PORTAL_URL) へリダイレクトする。支援者は決済時のメールアドレスで
// ログインし、解約・支払い方法の変更・領収書の確認を自分で行う。未設定なら 503「準備中」。
// /donation/checkout と同じ方式
export function GET(): Response {
  const url = donationLinkUrl("portal");
  if (!url) return donationUnavailableResponse("portal");
  redirect(url);
}
