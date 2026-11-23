import { redirect } from "next/navigation";
import { donationLinkUrl, donationUnavailableResponse } from "@/lib/donation";

// 支援ページ (/donation) の「毎月支援する」の飛び先。継続課金の Stripe Payment Links の URL
// (STRIPE_MONTHLY_DONATION_PAYMENT_LINK_URL) へリダイレクトする。未設定なら 503「準備中」。
// /donation/checkout と同じ方式
export function GET(): Response {
  const url = donationLinkUrl("monthly");
  if (!url) return donationUnavailableResponse("monthly");
  redirect(url);
}
